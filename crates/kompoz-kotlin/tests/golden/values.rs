::std::thread_local! { # [allow (non_snake_case , non_upper_case_globals , unused_parens , unused_braces , unused_mut , while_true , clippy :: all)] pub static Greeting : :: std :: string :: String = { # [allow (unused_imports)] use :: kompoz :: runtime :: kt :: ext :: * ; :: kompoz :: runtime :: named_args ! (:: kompoz :: runtime :: kt :: template ! ("selam " , Mark)) } ; }
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
const Mark: &str = "!";
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub const Steps: i32 = 3 * 2;
::std::thread_local! { # [allow (non_snake_case , non_upper_case_globals , unused_parens , unused_braces , unused_mut , while_true , clippy :: all)] pub static Purple : Color = { # [allow (unused_imports)] use :: kompoz :: runtime :: kt :: ext :: * ; :: kompoz :: runtime :: named_args ! (Color (0xFF6650A4_i64)) } ; }
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub static LocalSpacing: KompozitionLocal<Spacing> = {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    static_kompozition_local_of(|| Spacing())
};
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub static LocalAccount: KompozitionLocal<Account> = {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    kompozition_local_of(|| ::kompoz::runtime::kt::error("not provided"))
};
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub const default_label: &str = "etiket";
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Locals() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Text(::kompoz::runtime::kt::template!(
        Greeting.with(::core::clone::Clone::clone),
        " ",
        LocalSpacing.current(),
        " ",
        ::kompoz::runtime::kt::top_level!(OtherFilesVal)
    ));
    KompozitionLocalProvider(&[LocalAccount.provides(Account(::std::string::String::from("Ada")))], || {
        Text(::kompoz::runtime::kt::template!(LocalAccount.current().name(), default_label))
    });
    let saved = remember_saveable_state(&::kompoz::runtime::kt::top_level!(town_saver), || {
        mutable_state_of(Town(::std::string::String::from("İzmir")))
    });
    Text(::kompoz::runtime::kt::KtDelegate::get_value(&saved).name());
    KompozitionLocalProvider(&[LocalOtherFiles.provides(::std::string::String::from("verilen"))], || {
        Text(::kompoz::runtime::kt::top_level_with!(LocalOtherFiles, |__kz_v| __kz_v.current()))
    });
}
