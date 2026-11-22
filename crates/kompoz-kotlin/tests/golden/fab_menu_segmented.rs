#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Segments(pick: i32, bold: bool) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let picked = remember(|| mutable_state_of(pick));
    let checked = remember(|| mutable_state_of(bold));
    SingleChoiceSegmentedButtonRow(|this| {
        for i in ::kompoz::runtime::kt::until(0, 2) {
            this.SegmentedButton(
                ::kompoz::runtime::kt::KtDelegate::get_value(&picked) == i,
                {
                    let picked = ::kompoz::runtime::__private::capture(&picked);
                    let i = ::kompoz::runtime::__private::capture(&i);
                    move || {
                        ::kompoz::runtime::kt::KtMutableDelegate::set_value(&picked, i);
                    }
                },
                {
                    let __kz_f1: ::std::sync::Arc<dyn Shape> = SegmentedButtonDefaults.item_shape(i, 2);
                    __kz_f1
                },
                {
                    let i = ::kompoz::runtime::__private::capture(&i);
                    move || {
                        Text(::kompoz::runtime::kt::template!(i));
                    }
                },
            );
        }
    });
    MultiChoiceSegmentedButtonRow(|this| {
        this.SegmentedButton(
            ::kompoz::runtime::kt::KtDelegate::get_value(&checked),
            {
                let checked = ::kompoz::runtime::__private::capture(&checked);
                move |it| {
                    ::kompoz::runtime::kt::KtMutableDelegate::set_value(&checked, it);
                }
            },
            ::std::sync::Arc::new(SegmentedButtonDefaults::base_shape()),
            move || {
                Text("Kalın");
            },
        );
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Menu(open: bool) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let expanded = remember(|| mutable_state_of(open));
    let mut items = ::kompoz::runtime::kt::list_of([
        ::kompoz::runtime::kt::to(Icons::Filled::Add(), ::std::string::String::from("Ekle")),
        ::kompoz::runtime::kt::to(Icons::Filled::Edit(), ::std::string::String::from("Düzenle")),
    ]);
    FloatingActionButtonMenu(
        ::kompoz::runtime::kt::KtDelegate::get_value(&expanded),
        || {
            ToggleFloatingActionButton(
                checked = ::kompoz::runtime::kt::KtDelegate::get_value(&expanded),
                on_checked_change = |it| {
                    ::kompoz::runtime::kt::KtMutableDelegate::set_value(&expanded, it);
                },
                container_color = ToggleFloatingActionButtonDefaults.container_color_by_progress(),
                container_size = ToggleFloatingActionButtonDefaults.container_size_of(80.dp()),
                content = |this| {
                    let __kz_this = ::core::clone::Clone::clone(this);
                    let turned = this.checked_progress() > 0.5_f32;
                    VectorIcon(
                        Icons::Filled::Add(),
                        content_description = ::core::option::Option::Some(if turned {
                            ::std::string::String::from("Kapat")
                        } else {
                            ::std::string::String::from("Aç")
                        }),
                        modifier = Modifier
                            .animate_icon({
                                let __kz_f2: ::std::rc::Rc<dyn ::core::ops::Fn() -> f32> = ::std::rc::Rc::new({
                                    let __kz_this = ::kompoz::runtime::__private::capture(&__kz_this);
                                    move || __kz_this.checked_progress()
                                });
                                __kz_f2
                            })
                            .graphics_layer_block({
                                let __kz_this = ::kompoz::runtime::__private::capture(&__kz_this);
                                move |this1| {
                                    this1.set_rotation_z(45.0 * __kz_this.checked_progress());
                                }
                            }),
                    );
                },
            );
        },
        |this| {
            ::kompoz::runtime::kt::KtList::for_each_indexed(&items, |i, item| {
                this.FloatingActionButtonMenuItem(
                    on_click = || {
                        ::kompoz::runtime::kt::KtMutableDelegate::set_value(&expanded, false);
                    },
                    icon = || {
                        VectorIcon(::kompoz::runtime::kt::KtPair::first(&item), ::core::option::Option::None);
                    },
                    text = || {
                        Text(::kompoz::runtime::kt::KtPair::second(&item));
                    },
                    modifier = Modifier.on_key_event({
                        let i = ::kompoz::runtime::__private::capture(&i);
                        move |it| it.r#type() == KeyEventType::KeyDown && it.key() == Key::DirectionUp && i == 0
                    }),
                );
            });
        },
    );
}
