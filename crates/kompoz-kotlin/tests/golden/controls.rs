#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Controls() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let __kz_d1 = remember(|| ::kompoz::runtime::mutable_state_of!(true));
    let checked = __kz_d1.component1();
    let on_checked_change = __kz_d1.component2();
    let stroke_width_px = ::kompoz::runtime::kt::with(LocalDensity.current(), |this: &mut _| {
        ::kompoz::runtime::kt::floor(this.to_px(CheckboxDefaults::StrokeWidth))
    });
    Row(
        Modifier.toggleable_indication(
            value = checked,
            on_value_change = on_checked_change.clone(),
            role = ::core::option::Option::Some(Role::Checkbox),
            indication = ::core::option::Option::None,
            interaction_source = ::core::option::Option::None,
        ),
        content = || {
            Checkbox(checked, ::core::option::Option::None);
            Checkbox(
                checked = checked,
                on_checked_change = ::core::option::Option::Some({
                    let __kz_f2: ::std::rc::Rc<dyn ::core::ops::Fn(bool)> =
                        ::std::rc::Rc::new(on_checked_change.clone());
                    __kz_f2
                }),
                colors = CheckboxDefaults.colors(checked_color = Color::Blue),
            );
            TriStateCheckbox(
                state = ToggleableState::On,
                on_click = ::core::option::Option::None,
                colors = CheckboxDefaults.colors_of_parts(checked_box_color = Color::Blue),
            );
            StrokedCheckbox(
                checked,
                ::core::option::Option::Some({
                    let __kz_f3: ::std::rc::Rc<dyn ::core::ops::Fn(bool)> =
                        ::std::rc::Rc::new(on_checked_change.clone());
                    __kz_f3
                }),
                Stroke(width = stroke_width_px),
                Stroke(width = stroke_width_px),
                colors = CheckboxDefaults.colors(),
            );
            RadioButton(checked, ::core::option::Option::None);
            Switch(
                checked = checked,
                on_checked_change = ::core::option::Option::Some({
                    let __kz_f4: ::std::rc::Rc<dyn ::core::ops::Fn(bool)> = ::std::rc::Rc::new({
                        let on_checked_change = ::kompoz::runtime::__private::capture(&on_checked_change);
                        move |it: bool| {
                            on_checked_change(it);
                        }
                    });
                    __kz_f4
                }),
                thumb_content = ::core::option::Option::None,
            );
        },
    );
}
