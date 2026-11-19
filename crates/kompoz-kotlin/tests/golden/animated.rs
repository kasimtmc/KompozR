#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Visibilities(open: bool, count: i32) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(|| {
        ColumnAnimatedVisibility(
            visible = open,
            enter = fade_in() + expand_vertically(),
            exit = fade_out() + shrink_vertically(),
            label = ::std::string::String::from("details"),
            content = |this| {
                let _ = &this;
                Text("ayrıntılar");
            },
        );
        AnimatedContent(
            target_state = count,
            transition_spec = |this: &AnimatedContentTransitionScope<_>| {
                if this.target_state() > this.initial_state() {
                    slide_in_vertically(initial_offset_y = |it| it)
                        .together_with(slide_out_vertically(target_offset_y = |it| -it))
                } else {
                    this.using(
                        (slide_in_vertically(initial_offset_y = |it| -it)
                            .together_with(slide_out_vertically(target_offset_y = |it| it))),
                        ::core::option::Option::Some({
                            let __kz_f1: ::std::rc::Rc<dyn SizeTransform> = SizeTransform(
                                false,
                                move |_: IntSize, _: IntSize| -> ::std::sync::Arc<dyn FiniteAnimationSpec<IntSize>> {
                                    ::std::sync::Arc::new(spring(
                                        stiffness = Spring::StiffnessMediumLow,
                                        visibility_threshold =
                                            ::core::option::Option::Some(IntSize::VisibilityThreshold),
                                    ))
                                },
                            );
                            __kz_f1
                        }),
                    )
                }
            },
            label = ::std::string::String::from("count"),
            content = |this, n| {
                Text(
                    ::kompoz::runtime::kt::template!("sayı ", n),
                    modifier = this.animate_enter_exit(Modifier, enter = fade_in()),
                );
            },
        );
        Crossfade(
            target_state = open,
            label = ::std::string::String::from("page"),
            content = |p| {
                Text(if p { ::std::string::String::from("açık") } else { ::std::string::String::from("kapalı") });
            },
        );
        Row(|| {
            RowAnimatedVisibility(open, |this| {
                let _ = &this;
                Box(modifier = Modifier.size(10.dp()));
            });
        });
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Entering() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let shown = remember(|| {
        ::kompoz::runtime::kt::KtScope::apply(MutableTransitionState(false), |this| {
            this.set_target_state(true);
        })
    });
    Column(|| {
        ColumnAnimatedVisibilityWithState(
            visible_state = shown.clone(),
            enter = fade_in(animation_spec = ::std::sync::Arc::new(tween(duration_millis = 100)))
                + expand_vertically(animation_spec = ::std::sync::Arc::new(tween(duration_millis = 100))),
            exit = fade_out(animation_spec = ::std::sync::Arc::new(tween(duration_millis = 100))),
            content = |this| {
                Box(modifier = this
                    .animate_enter_exit(
                        Modifier,
                        enter = slide_in_vertically(
                            animation_spec = ::std::sync::Arc::new(tween(duration_millis = 100)),
                            initial_offset_y = |it| it,
                        ),
                        exit = scale_out(target_scale = 0.5_f32),
                    )
                    .size(10.dp())
                    .test_tag("giren"));
            },
        );
        Text(::kompoz::runtime::kt::template!("durum ", shown.current_state()));
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Pane(scope: AnimatedVisibilityScope, page: i32) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Box(modifier = ::kompoz::runtime::kt::with(scope.clone(), |this: &mut _| this.animate_enter_exit(Modifier))
        .size(10.dp())
        .test_tag(::kompoz::runtime::kt::template!("sayfa ", page)));
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Contents(page: i32) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    AnimatedContent(
        target_state = page,
        content_key = ::core::option::Option::Some({
            let __kz_f2: ::std::rc::Rc<dyn ::core::ops::Fn(&i32) -> SlotKey> = ::std::rc::Rc::new(move |it: &i32| {
                let it = ::core::clone::Clone::clone(it);
                ::kompoz::ui::layout::SlotKey::new(it.clone() / 2)
            });
            __kz_f2
        }),
        content = |this, p| {
            Pane(::core::convert::Into::into(this.clone()), p.clone());
        },
    );
}
