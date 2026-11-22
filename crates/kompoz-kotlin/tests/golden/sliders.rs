#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Sliders(bold: bool) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let price = remember(|| mutable_state_of(::core::ops::RangeInclusive::new(200_f32, 800_f32)));
    let whole: ::core::ops::RangeInclusive<f32> = ::core::ops::RangeInclusive::new(0_f32, 1000_f32);
    let picked = ::kompoz::runtime::kt::delegate_of::<::core::option::Option<i32>, _>(remember(|| {
        mutable_state_of(::core::option::Option::None)
    }));
    let stars =
        remember_slider_state(value = 3_f32, steps = 3, value_range = ::core::ops::RangeInclusive::new(1_f32, 5_f32));
    let hours = remember_range_slider_state(
        active_range_start = 9_f32,
        active_range_end = 17_f32,
        value_range = ::core::ops::RangeInclusive::new(0_f32, 24_f32),
    );
    stars.set_should_auto_snap(false);
    stars.set_on_value_change(::core::option::Option::Some({
        let __kz_f1: ::std::rc::Rc<dyn ::core::ops::Fn(f32)> = ::std::rc::Rc::new({
            let picked = ::kompoz::runtime::__private::capture(&picked);
            move |v: f32| {
                ::kompoz::runtime::kt::KtMutableDelegate::set_value(
                    &picked,
                    ::core::option::Option::Some(::kompoz::runtime::kt::KtFloat::round_to_int(v)),
                );
            }
        });
        __kz_f1
    }));
    Column(|| {
        RangeSlider(
            value = ::kompoz::runtime::kt::KtDelegate::get_value(&price),
            on_value_change = |it| {
                ::kompoz::runtime::kt::KtMutableDelegate::set_value(&price, it.clone());
            },
            value_range = whole.clone(),
            steps = 19,
        );
        StateSlider(
            state = stars.clone(),
            track = ::core::option::Option::Some({
                let __kz_f6: ::std::rc::Rc<dyn ::core::ops::Fn(SliderState)> = ::std::rc::Rc::new({
                    let bold = ::kompoz::runtime::__private::capture(&bold);
                    move |s: SliderState| {
                        SliderDefaults.Track(
                            s.clone(),
                            draw_stop_indicator = ::core::option::Option::Some(if bold {
                                ::core::option::Option::Some({
                                    let __kz_f5: ::std::rc::Rc<
                                        dyn ::core::ops::Fn(&mut ::kompoz::ui::draw::DrawScope, Offset),
                                    > = ::std::rc::Rc::new(
                                        move |this: &mut ::kompoz::ui::draw::DrawScope, it: Offset| {
                                            {
                                                let __kz_a2 = Color::Red;
                                                let __kz_a3 = this.to_px(3.dp());
                                                let __kz_a4 = it;
                                                this.draw_circle(__kz_a2, radius = __kz_a3, center = __kz_a4)
                                            };
                                        },
                                    );
                                    __kz_f5
                                })
                            } else {
                                ::core::option::Option::None
                            }),
                        );
                    }
                });
                __kz_f6
            }),
        );
        StateRangeSlider(
            state = hours.clone(),
            track = ::core::option::Option::Some({
                let __kz_f7: ::std::rc::Rc<dyn ::core::ops::Fn(RangeSliderState)> =
                    ::std::rc::Rc::new(move |it: RangeSliderState| {
                        SliderDefaults.RangeTrack(it.clone());
                    });
                __kz_f7
            }),
        );
        let inside = ::kompoz::runtime::kt::is_in(&500_f32, &::kompoz::runtime::kt::KtDelegate::get_value(&price))
            && !::kompoz::runtime::kt::KtClosedRange::is_empty(&whole);
        Text(::kompoz::runtime::kt::template!(
            ::kompoz::runtime::kt::KtFloat::round_to_int(::kompoz::runtime::kt::KtClosedRange::start(
                &::kompoz::runtime::kt::KtDelegate::get_value(&price)
            )),
            "–",
            ::kompoz::runtime::kt::KtFloat::round_to_int(::kompoz::runtime::kt::KtClosedRange::end_inclusive(
                &::kompoz::runtime::kt::KtDelegate::get_value(&price)
            )),
            " ",
            ::kompoz::runtime::kt::elvis(::kompoz::runtime::kt::KtDelegate::get_value(&picked), || 0),
            " ",
            inside
        ));
    });
}
