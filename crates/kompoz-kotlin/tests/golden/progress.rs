#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Progress() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(|| {
        CircularWavyProgressIndicator(
            progress = || 0.5_f32,
            amplitude = WavyProgressIndicatorDefaults::indicator_amplitude(),
        );
        LinearProgressIndicator(
            progress = || 0.5_f32,
            draw_stop_indicator = ::core::option::Option::Some({
                let __kz_f1: ::std::rc::Rc<dyn ::core::ops::Fn(&mut ::kompoz::ui::draw::DrawScope)> =
                    ::std::rc::Rc::new(move |this: &mut ::kompoz::ui::draw::DrawScope| {
                        ProgressIndicatorDefaults.draw_stop_indicator(&mut *this, 4.dp(), Color::Red, StrokeCap::Butt);
                    });
                __kz_f1
            }),
        );
        FloatLinearProgressIndicator(0.25_f32);
        IndeterminateLinearProgressIndicator();
        LinearWavyProgressIndicator(
            progress = || 0.5_f32,
            amplitude = {
                let __kz_f2: ::std::rc::Rc<dyn ::core::ops::Fn(f32) -> f32> = ::std::rc::Rc::new(move |_: f32| 0.5_f32);
                __kz_f2
            },
            wave_speed = ::core::option::Option::Some(0.dp()),
        );
    });
}
