#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Screen(flow: StateFlow<i32>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let shown = ::kompoz::runtime::key(&(6u32, 18u32), || remember(|| ::kompoz::runtime::mutable_state_of!(true)));
    let value = ::kompoz::runtime::key(&(7u32, 18u32), || flow.collect_as_state());
    ::kompoz::runtime::key(&(8u32, 5u32), || {
        Column(|| {
            if ::kompoz::runtime::kt::KtDelegate::get_value(&shown) {
                ::kompoz::runtime::key(&(9u32, 20u32), || {
                    Text(::kompoz::runtime::kt::template!(
                        "görünür ",
                        ::kompoz::runtime::kt::KtDelegate::get_value(&value)
                    ))
                });
            }
            ::kompoz::runtime::key(&(10u32, 9u32), || Text("hep"));
            ::kompoz::runtime::key(&(11u32, 9u32), || {
                LaunchedEffect(::kompoz::runtime::kt::KtDelegate::get_value(&shown), async {
                    ::kompoz::runtime::kt::println(::kompoz::runtime::kt::KtDelegate::get_value(&shown));
                })
            });
        })
    });
}
