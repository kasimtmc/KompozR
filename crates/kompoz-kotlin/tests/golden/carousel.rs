#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Browse(count: i32) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let state = remember_carousel_state({
        let count = ::kompoz::runtime::__private::capture(&count);
        move || count
    });
    HorizontalMultiBrowseCarousel(
        state = state.clone(),
        preferred_item_width = 186.dp(),
        item_spacing = 8.dp(),
        content = |this, i| {
            let __kz_this = ::core::clone::Clone::clone(this);
            let shape = remember(|| {
                GenericShape(move |this1: &mut Path, size: Size, _: LayoutDirection| {
                    this1.add_round_rect(
                        &RoundRect(0_f32, 0_f32, size.width(), size.height(), CornerRadius(30_f32)),
                        PathDirection::CounterClockwise,
                    );
                })
            });
            Box(
                this.mask_border(
                    this.mask_clip(Modifier.fill_max_size(), ::std::sync::Arc::new(shape.clone())),
                    BorderStroke(1.dp(), Color::Red),
                    ::std::sync::Arc::new(MaterialTheme::shapes().small()),
                )
                .graphics_layer_block({
                    let __kz_this = ::kompoz::runtime::__private::capture(&__kz_this);
                    move |this1| {
                        this1.set_translation_x(
                            __kz_this.carousel_item_draw_info().mask_rect().left() + this1.to_px(8.dp()),
                        );
                    }
                }),
                content = || {
                    Text(::kompoz::runtime::kt::template!(i));
                },
            );
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Aspects(widths: ::std::vec::Vec<Dp>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    MultiAspectCarouselScope(|this| {
        let __kz_this = ::core::clone::Clone::clone(this);
        let state = remember_lazy_list_state();
        LazyRow(
            state = ::core::option::Option::Some(state.clone()),
            content = |this1| {
                this1.items_indexed(widths.clone(), {
                    let state = ::kompoz::runtime::__private::capture(&state);
                    let __kz_this = ::kompoz::runtime::__private::capture(&__kz_this);
                    move |i, width| {
                        let width = ::core::clone::Clone::clone(width);
                        let draw_info = remember(|| MultiAspectCarouselItemDrawInfo(i, state.clone()));
                        Box(__kz_this.mask_clip(
                            Modifier.width(width),
                            ::std::sync::Arc::new(RectangleShape),
                            draw_info.clone(),
                        ));
                    }
                });
            },
        );
    });
}
