#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn PaintedIcons(tint: Color, filter: impl ::core::convert::Into<::core::option::Option<ColorFilter>>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let painter = remember_vector_painter(Icons::Filled::Favorite());
    Canvas(Modifier.size(48.dp()), |this| {
        ::kompoz::runtime::kt::with(painter.clone(), |this1: &mut _| {
            {
                let __kz_a1 = this.size();
                let __kz_a2 = DefaultAlpha;
                let __kz_a3 = ::core::option::Option::None;
                this1.draw(&mut *this, __kz_a1, __kz_a2, __kz_a3)
            };
            this1.draw(
                &mut *this,
                Size(24_f32, 24_f32),
                0.5_f32,
                ::core::option::Option::Some(&ColorFilter::tint(tint, BlendMode::SrcIn)),
            );
            {
                let __kz_a4 = this.size();
                let __kz_a5 = DefaultAlpha;
                let __kz_a6 = ::core::option::Option::as_ref(&filter);
                this1.draw(&mut *this, __kz_a4, __kz_a5, __kz_a6)
            };
            {
                let __kz_a7 = this.size();
                let __kz_a8 = DefaultAlpha;
                let __kz_a9 = ::core::option::Option::None;
                this1.draw(&mut *this, __kz_a7, __kz_a8, __kz_a9)
            }
        });
        this.translate(
            left = 8_f32,
            block = |this1| {
                ::kompoz::runtime::kt::with(painter.clone(), |this2: &mut _| {
                    let __kz_a10 = this1.size() / 2_f32;
                    let __kz_a11 = 1_f32;
                    let __kz_a12 = ::core::option::Option::Some(&ColorFilter::tint(tint, BlendMode::SrcIn));
                    this2.draw(&mut *this1, __kz_a10, __kz_a11, __kz_a12)
                });
            },
        );
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn TrackIcons(color: Color) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let icon = remember_vector_painter(Icons::Filled::Star());
    let draw_at = {
        let icon = ::kompoz::runtime::__private::capture(&icon);
        move |this: &mut ::kompoz::ui::draw::DrawScope, offset: Offset, c: Color| {
            this.translate(
                offset.x(),
                offset.y(),
                block = |this1| {
                    ::kompoz::runtime::kt::with(icon.clone(), |this2: &mut _| {
                        this2.draw(
                            &mut *this1,
                            Size(20_f32, 20_f32),
                            DefaultAlpha,
                            ::core::option::Option::Some(&ColorFilter::tint(c, BlendMode::SrcIn)),
                        )
                    });
                },
            );
        }
    };
    Box(Modifier.size(40.dp()).draw_with_content({
        let draw_at = ::kompoz::runtime::__private::capture(&draw_at);
        let color = ::kompoz::runtime::__private::capture(&color);
        let icon = ::kompoz::runtime::__private::capture(&icon);
        move |this| {
            this.draw_content();
            draw_at(this, Offset(0_f32, 10_f32), color);
            ::kompoz::runtime::kt::with(icon.clone(), |this1: &mut _| {
                let __kz_a13 = this.size();
                let __kz_a14 = 0.5_f32;
                let __kz_a15 = ::core::option::Option::None;
                this1.draw(&mut *this, __kz_a13, __kz_a14, __kz_a15)
            });
        }
    }));
}
