#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Photo(open: bool) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    SharedTransitionLayout(
        modifier = Modifier.fill_max_width(),
        content = |this| {
            let __kz_this = ::core::clone::Clone::clone(this);
            AnimatedContent(
                target_state = open,
                label = ::std::string::String::from("photo"),
                content = |this1, o| {
                    Box(modifier = __kz_this.shared_element(
                        Modifier.size(if o.clone() { 80.dp() } else { 40.dp() }).test_tag(if o.clone() {
                            ::std::string::String::from("büyük")
                        } else {
                            ::std::string::String::from("küçük")
                        }),
                        __kz_this.remember_shared_content_state(::kompoz::ui::layout::SlotKey::new(
                            ::std::string::String::from("photo"),
                        )),
                        &this1.clone(),
                    ));
                },
            );
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Item(
    id: i32,
    shared_transition_scope: SharedTransitionScope,
    animated_visibility_scope: AnimatedVisibilityScope,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::kt::with(shared_transition_scope.clone(), |this: &mut _| {
        Box(modifier = this
            .shared_bounds(
                Modifier,
                this.remember_shared_content_state(::kompoz::ui::layout::SlotKey::new(
                    ::kompoz::runtime::kt::template!("öğe ", id),
                )),
                &animated_visibility_scope.clone(),
                resize_mode = ResizeMode::RemeasureToBounds,
            )
            .size(40.dp()))
    });
}
