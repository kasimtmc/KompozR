#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn EveryTransition(shown: bool) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::key(&(18u32, 5u32), || {
        Column(|| {
            ::kompoz::runtime::key(&(19u32, 9u32), || {
                ColumnAnimatedVisibility(
                    visible = shown,
                    enter = slide_in(
                        ::std::sync::Arc::new(tween(320, easing = ::std::sync::Arc::new(LinearEasing))),
                        initial_offset = |it| IntOffset(it.width() * 2, 0),
                    ) + fade_in(initial_alpha = 0.3_f32),
                    exit = slide_out(
                        ::std::sync::Arc::new(tween(320, easing = ::std::sync::Arc::new(LinearEasing))),
                        target_offset = |it| IntOffset(-it.width(), 0),
                    ) + fade_out(target_alpha = 0.1_f32),
                    content = |this| {
                        let _ = &this;
                        ::kompoz::runtime::key(&(24u32, 13u32), || {
                            Box(modifier = Modifier.size_wh(100.dp(), 50.dp()).test_tag("kayan"))
                        });
                    },
                )
            });
            ::kompoz::runtime::key(&(26u32, 9u32), || {
                ColumnAnimatedVisibility(
                    visible = shown,
                    modifier = Modifier.test_tag("büyüyen"),
                    enter = expand_in(
                        ::std::sync::Arc::new(tween(320, easing = ::std::sync::Arc::new(LinearEasing))),
                        expand_from = Alignment::TopStart,
                        clip = false,
                        initial_size = |it| IntSize(it.width() / 2, it.height() / 2),
                    ),
                    exit = shrink_horizontally(
                        ::std::sync::Arc::new(tween(320, easing = ::std::sync::Arc::new(LinearEasing))),
                        shrink_towards = Alignment::Start,
                        target_width = |it| it / 4,
                    ),
                    content = |this| {
                        let _ = &this;
                        ::kompoz::runtime::key(&(32u32, 13u32), || Box(modifier = Modifier.size_wh(100.dp(), 50.dp())));
                    },
                )
            });
            ::kompoz::runtime::key(&(34u32, 9u32), || {
                ColumnAnimatedVisibility(
                    visible = shown,
                    enter = scale_in(initial_scale = 0.5_f32, transform_origin = TransformOrigin(0_f32, 0_f32))
                        + unveil_in(initial_color = Color::Red)
                        + expand_vertically(expand_from = Alignment::Top, initial_height = |it| it / 2)
                        + slide_in_horizontally(initial_offset_x = |it| -it)
                        + EnterTransition::None,
                    exit = scale_out(target_scale = 0.5_f32)
                        + veil_out(target_color = Color::Blue, match_parent_size = true)
                        + shrink_out(shrink_towards = Alignment::Center)
                        + slide_out_vertically(target_offset_y = |it| it)
                        + ExitTransition::None,
                    content = |this| {
                        let alpha = this.transition().animate_float(
                            label = ::std::string::String::from("kendi"),
                            target_value_by_state = |it| {
                                let it = ::core::clone::Clone::clone(it);
                                if it == EnterExitState::Visible { 1_f32 } else { 0_f32 }
                            },
                        );
                        ::kompoz::runtime::key(&(43u32, 13u32), || {
                            Box(modifier = Modifier
                                .size(20.dp())
                                .graphics_layer_block({
                                    let alpha = ::kompoz::runtime::__private::capture(&alpha);
                                    move |this1| {
                                        this1.set_alpha(::kompoz::runtime::kt::KtDelegate::get_value(&alpha));
                                    }
                                })
                                .test_tag("ölçekli"))
                        });
                    },
                )
            });
        })
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn OfATransition(step: i32) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let transition = ::kompoz::runtime::key(&(50u32, 22u32), || {
        update_transition(step, label = ::core::option::Option::Some(::std::string::String::from("adım")))
    });
    ::kompoz::runtime::key(&(51u32, 5u32), || {
        Column(|| {
            transition.AnimatedVisibility(
                visible = |it| {
                    let it = ::core::clone::Clone::clone(it);
                    it > 0
                },
                modifier = Modifier.test_tag("görünür"),
                enter = fade_in(animation_spec = ::std::sync::Arc::new(tween(duration_millis = 200))),
                exit = fade_out(animation_spec = ::std::sync::Arc::new(tween(duration_millis = 200))),
                content = |this| {
                    let _ = &this;
                    ::kompoz::runtime::key(&(53u32, 13u32), || Box(modifier = Modifier.size(20.dp())));
                },
            );
            transition.AnimatedContent(
                transition_spec = |this: &AnimatedContentTransitionScope<_>| {
                    let _ = &this;
                    fade_in(animation_spec = ::std::sync::Arc::new(tween(duration_millis = 160)))
                        .together_with(fade_out(animation_spec = ::std::sync::Arc::new(tween(duration_millis = 160))))
                },
                content = |this, s| {
                    let _ = &this;
                    ::kompoz::runtime::key(&(56u32, 13u32), || Text(::kompoz::runtime::kt::template!("adım ", s)));
                },
            );
            transition.Crossfade(
                animation_spec = ::std::sync::Arc::new(tween(duration_millis = 240)),
                content_key = ::core::option::Option::Some({
                    let __kz_f1: ::std::rc::Rc<dyn ::core::ops::Fn(&_) -> SlotKey> =
                        ::std::rc::Rc::new(move |it: &_| {
                            let it = ::core::clone::Clone::clone(it);
                            ::kompoz::ui::layout::SlotKey::new(it / 2)
                        });
                    __kz_f1
                }),
                content = |s| {
                    ::kompoz::runtime::key(&(58u32, 90u32), || Text(::kompoz::runtime::kt::template!("geçiş ", s)));
                },
            );
        })
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Sliding(state: SeekableTransitionState<i32>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let transition = ::kompoz::runtime::key(&(64u32, 22u32), || {
        remember_transition(state.clone(), label = ::core::option::Option::Some(::std::string::String::from("sayfa")))
    });
    transition.AnimatedContent(
        modifier = Modifier.test_tag("kap"),
        transition_spec = |this: &AnimatedContentTransitionScope<_>| {
            if this.target_state() > this.initial_state() {
                this.slide_into_container(
                    SlideDirection::Left,
                    animation_spec = ::std::sync::Arc::new(tween(320, easing = ::std::sync::Arc::new(LinearEasing))),
                )
                .together_with(this.slide_out_of_container(
                    SlideDirection::Left,
                    animation_spec = ::std::sync::Arc::new(tween(320, easing = ::std::sync::Arc::new(LinearEasing))),
                ))
            } else {
                ContentTransform(
                    fade_in(animation_spec = ::std::sync::Arc::new(tween(duration_millis = 320))),
                    ExitTransition::KeepUntilTransitionsFinished,
                    target_content_z_index = -1_f32,
                    size_transform = ::core::option::Option::None,
                )
            }
        },
        content_alignment = Alignment::Center,
        content = |this, page| {
            let _ = &this;
            ::kompoz::runtime::key(&(77u32, 9u32), || {
                Box(modifier =
                    Modifier.size_wh(100.dp(), 40.dp()).test_tag(::kompoz::runtime::kt::template!("parça ", page)))
            });
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Deferred(visible: DeferredTransitionState<bool>, page: DeferredTransitionState<i32>, drag: i32) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let shown = ::kompoz::runtime::key(&(83u32, 17u32), || remember_deferred_transition(visible.clone()));
    let transform = ::kompoz::runtime::key(&(84u32, 21u32), || remember(|| MutableTransform()));
    transform.update({
        let drag = ::kompoz::runtime::__private::capture(&drag);
        move |this, full_size| {
            let _ = &full_size;
            this.set_offset(IntOffset(drag, 0));
            this.set_alpha(1_f32 - (drag as f32) / 200_f32);
        }
    });
    ::kompoz::runtime::key(&(86u32, 5u32), || {
        Column(|| {
            shown.DeferredAnimatedVisibility(
                visible = |it| {
                    let it = ::core::clone::Clone::clone(it);
                    it
                },
                exit = slide_out(
                    ::std::sync::Arc::new(tween(duration_millis = 320)),
                    target_offset = |it| IntOffset(it.width() * 2, 0),
                ),
                mutable_transform = ::core::option::Option::Some(transform.clone()),
                content = |this| {
                    let _ = &this;
                    ::kompoz::runtime::key(&(88u32, 13u32), || {
                        Box(modifier = Modifier.size_wh(100.dp(), 40.dp()).test_tag("sürüklenen"))
                    });
                },
            );
            let pages = ::kompoz::runtime::key(&(90u32, 21u32), || remember_deferred_transition(page.clone()));
            pages.DeferredAnimatedContent(
                transition_spec = |this: &AnimatedContentTransitionScope<_>| {
                    let _ = &this;
                    fade_in(animation_spec = ::std::sync::Arc::new(tween(duration_millis = 200)))
                        .together_with(fade_out(animation_spec = ::std::sync::Arc::new(tween(duration_millis = 200))))
                },
                mutable_transform_spec = |this: &AnimatedContentTransitionScope<_>| {
                    let _ = &this;
                    ::core::option::Option::Some(MutableContentTransform(
                        block = |this1| {
                            this1.initial_content_transform({
                                let drag = ::kompoz::runtime::__private::capture(&drag);
                                move |this2, _| {
                                    this2.set_offset(IntOffset(-drag, 0));
                                }
                            });
                            this1.target_content_transform({
                                let drag = ::kompoz::runtime::__private::capture(&drag);
                                move |this2, full_size| {
                                    this2.set_offset(IntOffset(full_size.width() - drag, 0));
                                }
                            });
                        },
                    ))
                },
                content = |this, n| {
                    let _ = &this;
                    ::kompoz::runtime::key(&(100u32, 13u32), || {
                        Box(modifier =
                            Modifier.size_wh(100.dp(), 40.dp()).test_tag(::kompoz::runtime::kt::template!("sayfa ", n)))
                    });
                },
            );
        })
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Pane(scope: AnimatedVisibilityScope, page: i32) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::key(&(107u32, 5u32), || {
        Box(modifier = ::kompoz::runtime::kt::with(scope.clone(), |this: &mut _| {
            this.animate_enter_exit(Modifier, enter = slide_in_horizontally(initial_offset_x = |it| it))
        })
        .size_wh(100.dp(), 40.dp())
        .test_tag(::kompoz::runtime::kt::template!("bölme ", page)))
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Panes(page: i32) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::key(&(113u32, 5u32), || {
        AnimatedContent(
            target_state = page,
            content_key = ::core::option::Option::Some({
                let __kz_f2: ::std::rc::Rc<dyn ::core::ops::Fn(&i32) -> SlotKey> =
                    ::std::rc::Rc::new(move |it: &i32| {
                        let it = ::core::clone::Clone::clone(it);
                        ::kompoz::ui::layout::SlotKey::new(it.clone() / 2)
                    });
                __kz_f2
            }),
            content = |this, p| {
                ::kompoz::runtime::key(&(114u32, 9u32), || Pane(::core::convert::Into::into(this.clone()), p.clone()));
            },
        )
    });
}
