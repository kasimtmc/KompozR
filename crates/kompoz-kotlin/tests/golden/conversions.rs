#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Conversions(
    log: impl ::core::ops::Fn(::std::string::String) + 'static,
    on_lines: impl ::core::ops::Fn(i32) + 'static,
    # [default (:: std :: sync :: Arc :: new (RoundedCornerShape (4 . dp ())))] shape: ::std::sync::Arc<dyn Shape>,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let points = remember(|| ::core::convert::identity::<SnapshotStateList<Offset>>(mutable_state_list_of([])));
    Row(
        horizontal_arrangement = AbsoluteArrangement::Left,
        content = || {
            Box(Modifier
                .combined_clickable(
                    on_long_click = ::core::option::Option::Some({
                        let __kz_f1: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new({
                            let log = ::kompoz::runtime::__private::capture(&log);
                            move || {
                                log(::std::string::String::from("long"));
                            }
                        });
                        __kz_f1
                    }),
                    on_click = || {
                        log(::std::string::String::from("click"));
                    },
                )
                .background(Color::Red, shape = shape.clone()));
            BasicText(
                "metin",
                on_text_layout = ::core::option::Option::Some({
                    let __kz_f2: ::std::rc::Rc<dyn ::core::ops::Fn(&TextLayoutResult)> = ::std::rc::Rc::new({
                        let on_lines = ::kompoz::runtime::__private::capture(&on_lines);
                        move |it: &TextLayoutResult| {
                            on_lines((it.line_count() as i32));
                        }
                    });
                    __kz_f2
                }),
            );
            Canvas(Modifier.size(10.dp()), |this| {
                this.draw_circle(
                    Color::Red,
                    radius = 4_f32,
                    style = ::core::convert::Into::into(Stroke(width = 2_f32)),
                );
                this.draw_line(
                    Color::Red,
                    Offset(0_f32, 0_f32),
                    Offset(1_f32, 1_f32),
                    path_effect = ::core::option::Option::Some(PathEffect::dash_path_effect(
                        &::kompoz::runtime::kt::float_array_of([2_f32, 1_f32]),
                        0_f32,
                    )),
                );
            });
            Text(::kompoz::runtime::kt::template!(points.size()));
        },
    );
}
