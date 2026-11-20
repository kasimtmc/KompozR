#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub async fn later(n: i32) -> i32 {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        delay(100).await;
        return n;
    })
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Built(log: impl ::core::ops::Fn(AnnotatedString) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let plain = build_annotated_string(|this| {
        this.append(::std::string::String::from("a"));
        this.with_style(SpanStyle(color = Color::Red), |this1| this1.append(::std::string::String::from("b")));
        this.with_link(LinkAnnotation::url("https://kompoz.dev"), |this1| {
            this1.append(::std::string::String::from("c"))
        });
    });
    log(plain.clone());
    LaunchedEffect((), async {
        let built = build_annotated_string_async(async |this| {
            this.append(::std::string::String::from("x"));
            this.with_style_async(SpanStyle(color = Color::Blue), async |this1| {
                this1.append(::kompoz::runtime::kt::template!(later(1).await));
                this1
                    .with_link_async(LinkAnnotation::url("https://kompoz.dev"), async |this2| {
                        delay(50).await;
                        this2.append(::std::string::String::from("y"))
                    })
                    .await
            })
            .await;
            this.with_style(SpanStyle(font_size = 12.sp()), |this1| this1.append(::std::string::String::from("z")));
            this.with_style(ParagraphStyle(text_align = TextAlign::Center), |this1| {
                this1.append(::std::string::String::from("p"))
            });
        })
        .await;
        log(built.clone());
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Values(state: TextFieldState, mutex: MutatorMutex, log: impl ::core::ops::Fn(::std::string::String) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    LaunchedEffect((), async {
        let color = Color::Unspecified
            .take_or_else_async(async || {
                delay(10).await;
                Color::Green
            })
            .await;
        let width = Dp::Unspecified.take_or_else_async(async || later(3).await.dp()).await;
        let size = TextUnit::Unspecified.take_or_else(|| 7.sp());
        state
            .edit_async(async |this| {
                this.append("b");
                delay(100).await;
                this.append("c");
            })
            .await;
        let ran = mutex
            .try_mutate_async(async || {
                delay(100).await;
            })
            .await;
        log(::kompoz::runtime::kt::template!(color, " ", width, " ", size, " ", ran));
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Vectors(
    nodes: impl ::core::ops::Fn(::std::vec::Vec<PathNode>) + 'static,
    icon: impl ::core::ops::Fn(ImageVector) + 'static,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    LaunchedEffect((), async {
        nodes(
            PathData_async(async |this| {
                this.move_to(0_f32, 0_f32);
                delay(10).await;
                this.line_to(4_f32, 4_f32);
            })
            .await,
        );
        icon(
            ImageVector::Builder("v", 8.dp(), 8.dp(), 8_f32, 8_f32, Color::Unspecified, BlendMode::SrcIn, false)
                .group_async(
                    name = "g",
                    rotate = 45_f32,
                    block = async |this| {
                        delay(10).await;
                        this.path_async(
                            fill = ::core::option::Option::Some(SolidColor(Color::Red)),
                            path_builder = async |this1| {
                                this1.move_to(0_f32, 0_f32);
                                delay(10).await;
                                this1.line_to(8_f32, 8_f32);
                            },
                        )
                        .await;
                    },
                )
                .await
                .build(),
        );
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
pub async fn paint(scope: DrawScope, log: impl ::core::ops::Fn(::std::string::String) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::named_args!({
        ::kompoz::runtime::kt::with_async(scope.clone(), async |this: &mut _| {
            this.translate_async(
                left = 2_f32,
                block = async |this1| {
                    delay(10).await;
                    this1.draw_rect(Color::Red, size = Size(1_f32, 1_f32));
                },
            )
            .await;
            this.with_transform_async(
                async |this1| {
                    this1.translate(1_f32, top = 1_f32);
                },
                async |this1| {
                    delay(10).await;
                    this1.draw_rect(Color::Blue);
                },
            )
            .await;
            this.with_transform_async(
                async |this1| {
                    delay(10).await;
                    this1.rotate(90_f32);
                },
                async |this1| {
                    this1.draw_rect(Color::Green);
                },
            )
            .await;
            this.clip_rect_async(
                right = 5_f32,
                block = async |this1| {
                    this1
                        .rotate_async(45_f32, async |this2| {
                            this2
                                .draw_into_canvas_async(async |it| {
                                    delay(1).await;
                                    it.draw_rect(Rect((0_f32, 0_f32, 1_f32, 1_f32)), &Paint());
                                })
                                .await;
                        })
                        .await;
                },
            )
            .await;
            this.inset_async(1_f32, 2_f32, 3_f32, 4_f32, async |this1| {
                this1
                    .scale_async(2_f32, 3_f32, async |this2| {
                        delay(1).await;
                        this2.draw_rect(Color::Red);
                    })
                    .await;
            })
            .await;
            this.with_transform(
                |this1| {
                    this1.translate(1_f32, top = 1_f32);
                },
                |this1| {
                    this1.draw_rect(Color::Blue);
                },
            )
        })
        .await;
        log(::std::string::String::from("painted"));
    })
}
// error 121:17: `delay` suspends, in a lambda `withAnnotation` doesn't run in the coroutine: Kotlin suspends in a coroutine and in the lambdas an inline function runs in place (`with`, `let`, `apply`, `forEach`, `map`, `repeat`, …; not a `crossinline` one: `sortedBy`'s)
// error 127:17: `withStyle(ParagraphStyle)`'s lambda can't suspend: Kotlin's `withStyle(ParagraphStyle)` takes it `crossinline` (`delay` suspends in it; only `withStyle(SpanStyle)`'s may)
// error 130:40: `delay` suspends, in the block `withMutableSnapshot` runs with a snapshot current: Compose's snapshot blocks must not suspend (the snapshot is the thread's current one until the block returns: the coroutines run meanwhile would run in it)
// error 131:45: `later` suspends, in the block `withoutReadObservation` runs with a snapshot current: Compose's snapshot blocks must not suspend (the snapshot is the thread's current one until the block returns: the coroutines run meanwhile would run in it)
