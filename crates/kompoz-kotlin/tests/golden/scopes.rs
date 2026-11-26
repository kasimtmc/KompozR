#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
# [:: kompoz :: runtime :: named (pub trait ModifierTallerByExt)]
impl Modifier {
    fn taller_by(self, extra: Dp) -> Modifier {
        #[allow(unused_imports)]
        use ::kompoz::runtime::kt::ext::*;
        ::kompoz::runtime::named_args!({
            self.clone().r#layout({
                let extra = ::kompoz::runtime::__private::capture(&extra);
                move |this1, measurable, constraints| {
                    let placeable = measurable.r#measure(constraints);
                    let more = this1.round_to_px(extra);
                    this1.layout(placeable.width(), placeable.height() + more, {
                        let placeable = ::kompoz::runtime::__private::capture(&placeable);
                        let more = ::kompoz::runtime::__private::capture(&more);
                        move |this2| {
                            this2.place_relative(&placeable, 0, more);
                        }
                    })
                }
            })
        })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Scopes() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let todos = remember(|| {
        ::kompoz::runtime::mutable_state_of!(::kompoz::runtime::kt::mutable_list_of([::std::string::String::from(
            "bir"
        )]))
    });
    let tags = remember(|| mutable_state_list_of([::std::string::String::from("a")]));
    Canvas(
        Modifier
            .size(40.dp())
            .clickable({
                let todos = ::kompoz::runtime::__private::capture(&todos);
                let tags = ::kompoz::runtime::__private::capture(&tags);
                move || {
                    todos.update(|it| {
                        ::kompoz::runtime::kt::KtMutableList::add(&mut *it, ::std::string::String::from("iki"));
                    });
                    tags.add(::std::string::String::from("b"));
                }
            })
            .pointer_input((), {
                let tags = ::kompoz::runtime::__private::capture(&tags);
                move |this| {
                    let tags = ::kompoz::runtime::__private::capture(&tags);
                    async move {
                        this.detect_drag_gestures({
                            let tags = ::kompoz::runtime::__private::capture(&tags);
                            move |change, _| {
                                tags.add(::kompoz::runtime::kt::template!(change.position()));
                            }
                        })
                        .on_drag_start({
                            let tags = ::kompoz::runtime::__private::capture(&tags);
                            move |_| {
                                tags.clear();
                            }
                        })
                        .await;
                    }
                }
            }),
        |this| {
            this.rotate(45_f32, |this1| {
                {
                    let __kz_a1 = Color::Blue;
                    let __kz_a2 = this1.center().x() / 4.0;
                    this1.draw_circle(__kz_a1, radius = __kz_a2)
                };
            });
        },
    );
}
