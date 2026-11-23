::std::thread_local! { # [allow (non_snake_case , non_upper_case_globals , unused_parens , unused_braces , unused_mut , while_true , clippy :: all)] pub static Gap : Dp = { # [allow (unused_imports)] use :: kompoz :: runtime :: kt :: ext :: * ; :: kompoz :: runtime :: named_args ! (8 . dp ()) } ; }
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub const Limit: i32 = 3;
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub const Title: &str = "Başlık";
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
const Ratio: f32 = 1.5_f32 * 2.0;
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn double(x: i32) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ x * 2 })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::named]
pub fn greet(
    name: impl ::core::convert::Into<::std::string::String>,
    #[default(::std::string::String::from("!"))] punctuation: ::std::string::String,
) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let name: ::std::string::String = ::core::convert::Into::into(name);
    ::kompoz::runtime::named_args!({
        return ::kompoz::runtime::kt::template!("Merhaba, ", name, punctuation);
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub(crate) fn sum(values: ::std::vec::Vec<i32>) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        let mut total = 0;
        for v in ::kompoz::runtime::kt::KtIterable::kt_iter(&values) {
            total += v;
        }
        return total;
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn label(n: i32) -> ::std::string::String {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        if n > Limit { ::std::string::String::from("çok") } else { ::std::string::String::from("az") }
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn log(message: impl ::core::convert::Into<::std::string::String>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let message: ::std::string::String = ::core::convert::Into::into(message);
    ::kompoz::runtime::named_args!({
        ::kompoz::runtime::kt::println(message.clone());
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub async fn pause(ms: i64) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        delay(ms).await;
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
# [:: kompoz :: runtime :: named (pub trait ModifierFramedExt)]
impl Modifier {
    fn framed(self, # [default (2 . dp ())] width: Dp) -> Modifier {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ self.clone().padding(width).border(width, Color::Black) })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
# [:: kompoz :: runtime :: named (pub trait ModifierCardExt)]
impl Modifier {
    fn card(self) -> Modifier {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({ self.clone().framed().padding(Gap.with(::core::clone::Clone::clone)) })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
# [:: kompoz :: runtime :: named (pub (self) trait StringShoutExt)]
impl ::std::string::String {
    fn shout(self) -> ::std::string::String {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            ::kompoz::runtime::kt::template!(::kompoz::runtime::kt::KtStr::uppercase(&self), "!")
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn twice(f: impl ::core::ops::Fn() + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        f();
        f();
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn apply2(x: i32, f: impl ::core::ops::Fn(i32) -> i32 + 'static) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ f(f(x)) })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn arrows() -> ::std::vec::Vec<Key> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ ::kompoz::runtime::kt::list_of([Key::DirectionLeft, Key::MoveEnd]) })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn step_sizes() -> ::std::rc::Rc<dyn TextAutoSize> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ <dyn TextAutoSize>::step_based(8.sp(), 40.sp(), 1.sp()) })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn two_anchors() -> ::std::sync::Arc<dyn DraggableAnchors<i32>> {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        ::std::sync::Arc::new(DraggableAnchors(|this: &mut DraggableAnchorsConfig<_>| {
            this.at(1, 0_f32);
            this.at(2, 120_f32);
        }))
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn math_forms() -> bool {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        ::kompoz::runtime::kt::round(2.5_f32) == 2_f32
            && ::kompoz::runtime::kt::KtNumber::to_float(((-3) as i32)) == -3_f32
            && ::kompoz::runtime::kt::KtNumber::to_float(::kompoz::runtime::kt::PI) > 3_f32
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn heart_to_pill() -> Morph {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({ Morph::new(MaterialShapes::Heart().clone(), MaterialShapes::Pill().clone()) })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub fn ten_long() -> f32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        let mut path = Path::new();
        path.line_to(10_f32, 0_f32);
        let mut measure = PathMeasure::new();
        measure.set_path(&path.clone(), false);
        return measure.length();
    })
}
