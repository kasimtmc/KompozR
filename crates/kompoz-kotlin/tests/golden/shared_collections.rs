#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn drop_first(xs: ::kompoz::runtime::kt::MutableList<i32>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        xs.remove_at(0);
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn tally(
    counts: ::kompoz::runtime::kt::MutableMap<::std::string::String, i32>,
    key: impl ::core::convert::Into<::std::string::String>,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let key: ::std::string::String = ::core::convert::Into::into(key);
    ::kompoz::runtime::named_args!({
        counts.set(key.clone(), (::kompoz::runtime::kt::elvis(counts.get(key.clone()), || 0)) + 1);
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn total(xs: ::std::vec::Vec<i32>) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::KtList::sum(&xs) })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn pop_if_mutable<T: ::core::clone::Clone + 'static>(stack: ::kompoz::runtime::kt::ListView<T>) -> bool {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        if let ::core::option::Option::Some(stack) = stack.as_mutable() {
            stack.remove_at(stack.size() - 1);
            return true;
        }
        return false;
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzBasket {
    items: ::kompoz::runtime::kt::KtVal<::kompoz::runtime::kt::MutableList<::std::string::String>>,
    _log: ::kompoz::runtime::kt::KtVal<::kompoz::runtime::kt::MutableList<::std::string::String>>,
    log: ::kompoz::runtime::kt::KtVal<::kompoz::runtime::kt::ListView<::std::string::String>>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Basket {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzBasket>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Basket(
    # [default (:: kompoz :: runtime :: kt :: mutable_list_of ([]))] items: ::kompoz::runtime::kt::MutableList<
        ::std::string::String,
    >,
) -> Basket {
    Basket::__kz_new(items)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Basket {
    #[doc(hidden)]
    pub fn __kz_new(items: ::kompoz::runtime::kt::MutableList<::std::string::String>) -> Basket {
        let __kz_this = Basket {
            __kz_this: ::std::rc::Rc::new(__KzBasket {
                items: ::kompoz::runtime::kt::KtVal::new(),
                _log: ::kompoz::runtime::kt::KtVal::new(),
                log: ::kompoz::runtime::kt::KtVal::new(),
            }),
        };
        __kz_this.__kz_init(items);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, items: ::kompoz::runtime::kt::MutableList<::std::string::String>) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        self.__kz_this.items.init(::core::clone::Clone::clone(&items));
        ::kompoz::runtime::named_args!({
            self.__kz_this._log.init(::core::convert::identity::<
                ::kompoz::runtime::kt::MutableList<::std::string::String>,
            >(::kompoz::runtime::kt::mutable_list_of([])));
            self.__kz_this.log.init(::kompoz::runtime::kt::ListView::from(self._log()));
        })
    }
    pub fn items(&self) -> ::kompoz::runtime::kt::MutableList<::std::string::String> {
        self.__kz_this.items.get()
    }
    fn _log(&self) -> ::kompoz::runtime::kt::MutableList<::std::string::String> {
        self.__kz_this._log.get()
    }
    pub fn log(&self) -> ::kompoz::runtime::kt::ListView<::std::string::String> {
        self.__kz_this.log.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Basket {
    pub fn put(&self, item: impl ::core::convert::Into<::std::string::String>) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        let item: ::std::string::String = ::core::convert::Into::into(item);
        ::kompoz::runtime::named_args!({
            self.items().add(item.clone());
            self._log().add(item.clone());
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for Basket {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Basket {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Basket {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Basket {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Basket",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Basket {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Basket {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn shared() -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        let xs = ::kompoz::runtime::kt::mutable_list_of([1, 2, 3]);
        let ys = xs.clone();
        drop_first(xs.clone());
        ys.add(4);
        ys.remove(2);
        xs.set(0, 9);
        let view: ::kompoz::runtime::kt::ListView<i32> = ::kompoz::runtime::kt::ListView::from(xs.clone());
        let copy = ::kompoz::runtime::kt::KtList::to_mutable_list(&xs);
        let seen = ::kompoz::runtime::kt::mutable_set_of([::std::string::String::from("a")]);
        seen.add_all(::kompoz::runtime::kt::KtList::to_list(&::kompoz::runtime::kt::list_of([
            ::std::string::String::from("b"),
            ::std::string::String::from("c"),
        ])));
        let counts = ::core::convert::identity::<::kompoz::runtime::kt::MutableMap<::std::string::String, i32>>(
            ::kompoz::runtime::kt::hash_map_of([]),
        );
        tally(counts.clone(), "k");
        {
            let __kz_g5 = ::kompoz::runtime::kt::to(::std::string::String::from("m"), 2);
            counts.put(__kz_g5.0, __kz_g5.1);
        }
        let sum = total(xs.clone().to_vec()) + view.size();
        return ::kompoz::runtime::kt::template!(
            xs,
            " ",
            copy,
            " ",
            seen,
            " ",
            counts,
            " ",
            sum,
            " ",
            xs.same(&ys),
            " ",
            xs.same(&copy),
            " ",
            pop_if_mutable(::kompoz::runtime::kt::ListView::from(xs.clone()))
        );
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn meta(
    value: i32,
    on_done: impl ::core::ops::Fn() + 'static,
) -> ::kompoz::runtime::kt::KtMap<::std::string::String, ::std::rc::Rc<dyn ::kompoz::runtime::kt::KtObject>> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        ::kompoz::runtime::kt::map_of([
            ::kompoz::runtime::kt::to(::std::string::String::from("value"), {
                let __kz_any6: ::std::rc::Rc<dyn ::kompoz::runtime::kt::KtObject> = ::std::rc::Rc::new(value);
                __kz_any6
            }),
            ::kompoz::runtime::kt::to(::std::string::String::from("done"), {
                let __kz_any7: ::std::rc::Rc<dyn ::kompoz::runtime::kt::KtObject> = ::std::rc::Rc::new(on_done);
                __kz_any7
            }),
        ])
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::named]
pub fn scaled(
    #[allow(unused_variables)] a: i32,
    #[allow(unused_variables)]
    # [default ({ let a = * a ; a * 2 })]
    b: i32,
    # [default ({ let a = * a ; let b = * b ; :: kompoz :: runtime :: kt :: template ! (a , "/" , b) })]
    label: ::std::string::String,
) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::template!(a, " ", b, " ", label) })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::named]
pub fn popper<T: ::core::clone::Clone + 'static>(
    #[allow(unused_variables)] stack: ::kompoz::runtime::kt::ListView<T>,
    # [default ({ let stack = :: core :: clone :: Clone :: clone (stack) ; { let stack = :: kompoz :: runtime :: __private :: capture (& stack) ; move | | { if let :: core :: option :: Option :: Some (stack) = stack . as_mutable () { stack . remove_at (stack . size () - 1) ; } } } })]
    on_back: impl ::core::ops::Fn() + 'static,
) -> impl ::core::ops::Fn() + 'static {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ on_back })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::named]
pub fn faded(
    #[allow(unused_variables)] duration: i32,
    # [default ({ let duration = * duration ; :: kompoz :: runtime :: named_args ! ({ ContentTransform (fade_in (animation_spec = :: std :: sync :: Arc :: new (tween (duration_millis = duration))) , fade_out () , size_transform = :: core :: option :: Option :: None) }) })]
    spec: ContentTransform,
) -> ContentTransform {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ spec.clone() })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[doc(hidden)]
pub struct __KzFrame {
    width: ::kompoz::runtime::kt::KtVal<i32>,
    height: ::kompoz::runtime::kt::KtVal<i32>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[derive(:: core :: clone :: Clone)]
pub struct Frame {
    #[doc(hidden)]
    pub __kz_this: ::std::rc::Rc<__KzFrame>,
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named(type)]
pub fn Frame(
    #[allow(unused_variables)] width: i32,
    # [default ({ let width = * width ; width / 2 })] height: i32,
) -> Frame {
    Frame::__kz_new(width, height)
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl Frame {
    #[doc(hidden)]
    pub fn __kz_new(width: i32, height: i32) -> Frame {
        let __kz_this = Frame {
            __kz_this: ::std::rc::Rc::new(__KzFrame {
                width: ::kompoz::runtime::kt::KtVal::new(),
                height: ::kompoz::runtime::kt::KtVal::new(),
            }),
        };
        __kz_this.__kz_init(width, height);
        __kz_this
    }
    #[doc(hidden)]
    #[allow(unused_variables)]
    fn __kz_init(&self, width: i32, height: i32) {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        self.__kz_this.width.init(width);
        self.__kz_this.height.init(height);
        ::kompoz::runtime::named_args!({})
    }
    pub fn width(&self) -> i32 {
        self.__kz_this.width.get()
    }
    pub fn height(&self) -> i32 {
        self.__kz_this.height.get()
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
#[::kompoz::runtime::named]
impl Frame {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::PartialEq for Frame {
    fn eq(&self, other: &Self) -> bool {
        ::std::rc::Rc::ptr_eq(&self.__kz_this, &other.__kz_this)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::cmp::Eq for Frame {}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::hash::Hash for Frame {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::ptr::hash(::std::rc::Rc::as_ptr(&self.__kz_this), state)
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::kompoz::runtime::kt::KtToString for Frame {
    fn kt_write(&self, __kz_out: &mut ::std::string::String) {
        __kz_out.push_str(&::kompoz::runtime::kt::default_to_string(
            "Frame",
            ::std::rc::Rc::as_ptr(&self.__kz_this) as *const (),
        ))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Display for Frame {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[allow(dead_code)]
impl ::core::fmt::Debug for Frame {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str(&::kompoz::runtime::kt::KtToString::kt_to_string(self))
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Titled(
    #[allow(unused_variables)] title: ::std::string::String,
    # [default ({ let title = :: core :: clone :: Clone :: clone (title) ; :: kompoz :: runtime :: kt :: KtStr :: uppercase (& title) })]
    subtitle: ::std::string::String,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Text(::kompoz::runtime::kt::template!(title, " ", subtitle));
}
