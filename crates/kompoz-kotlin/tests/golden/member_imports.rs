#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Imported(expanded: bool) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let below = FloatingToolbarDefaults.exit_always_scroll_behavior(FloatingToolbarExitDirection::Bottom);
    let beside = FloatingToolbarDefaults.exit_always_scroll_behavior(FloatingToolbarExitDirection::End);
    Row(
        horizontal_arrangement = AbsoluteArrangement::Left,
        content = || {
            HorizontalFloatingToolbar(
                expanded,
                Modifier.offset(y = -FloatingToolbarDefaults::ScreenOffset),
                scroll_behavior = ::core::option::Option::Some({
                    let __kz_f1: ::std::rc::Rc<dyn FloatingToolbarScrollBehavior> = below.clone();
                    __kz_f1
                }),
                content = || {
                    Text("a");
                },
            );
            VerticalFloatingToolbar(
                expanded,
                Modifier.offset(x = -FloatingToolbarDefaults::ScreenOffset),
                scroll_behavior = ::core::option::Option::Some({
                    let __kz_f2: ::std::rc::Rc<dyn FloatingToolbarScrollBehavior> = beside.clone();
                    __kz_f2
                }),
                content = || {
                    Text("b");
                },
            );
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Shadowed() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let ScreenOffset = 4.dp();
    Box(Modifier.padding(ScreenOffset));
}
