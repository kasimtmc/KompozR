#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Pulled() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let pulled = remember(|| mutable_state_of(Offset::Zero));
    let panned = remember(|| mutable_state_of(Offset::Zero));
    let drag = remember_draggable2d_state({
        let pulled = ::kompoz::runtime::__private::capture(&pulled);
        move |it| {
            ::kompoz::runtime::kt::KtMutableDelegate::set_value(
                &pulled,
                Offset(
                    ::kompoz::runtime::kt::KtDelegate::get_value(&pulled).x() + it.x(),
                    ::kompoz::runtime::kt::KtDelegate::get_value(&pulled).y() + it.y(),
                ),
            );
        }
    });
    let pan = remember_scrollable2d_state({
        let panned = ::kompoz::runtime::__private::capture(&panned);
        move |d| {
            ::kompoz::runtime::kt::KtMutableDelegate::set_value(
                &panned,
                Offset(
                    ::kompoz::runtime::kt::KtDelegate::get_value(&panned).x()
                        + ::kompoz::runtime::kt::min(d.x(), 0_f32),
                    ::kompoz::runtime::kt::KtDelegate::get_value(&panned).y()
                        + ::kompoz::runtime::kt::max(d.y(), 0_f32),
                ),
            );
            Offset(::kompoz::runtime::kt::min(d.x(), 0_f32), ::kompoz::runtime::kt::max(d.y(), 0_f32))
        }
    });
    Column(|| {
        Box(modifier =
            Modifier.size(100.dp()).draggable2d(drag.clone(), start_drag_immediately = true).test_tag("sürüklenir"));
        Box(modifier = Modifier.size(100.dp()).scrollable2d(pan.clone()).test_tag("kayar"));
        Text(::kompoz::runtime::kt::template!(
            "çekilen ",
            ::kompoz::runtime::kt::KtFloat::round_to_int(::kompoz::runtime::kt::KtDelegate::get_value(&pulled).x()),
            " ",
            ::kompoz::runtime::kt::KtFloat::round_to_int(::kompoz::runtime::kt::KtDelegate::get_value(&pulled).y()),
            " kaydırılan ",
            ::kompoz::runtime::kt::KtFloat::round_to_int(::kompoz::runtime::kt::KtDelegate::get_value(&panned).x()),
            " ",
            ::kompoz::runtime::kt::KtFloat::round_to_int(::kompoz::runtime::kt::KtDelegate::get_value(&panned).y())
        ));
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Zoom() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let scale = remember(|| mutable_state_of(1_f32));
    let moved = remember(|| mutable_state_of(0_f32));
    let state = remember_transformable_state({
        let scale = ::kompoz::runtime::__private::capture(&scale);
        let moved = ::kompoz::runtime::__private::capture(&moved);
        move |_, zoom, pan, _| {
            ::kompoz::runtime::kt::KtMutableDelegate::set_value(
                &scale,
                ::kompoz::runtime::kt::KtDelegate::get_value(&scale) * zoom,
            );
            ::kompoz::runtime::kt::KtMutableDelegate::set_value(
                &moved,
                ::kompoz::runtime::kt::KtDelegate::get_value(&moved) + pan.y(),
            );
        }
    });
    Column(|| {
        Box(modifier = Modifier
            .size(300.dp())
            .transformable_with_can_pan(state.clone(), {
                let scale = ::kompoz::runtime::__private::capture(&scale);
                move |_| ::kompoz::runtime::kt::KtDelegate::get_value(&scale) > 1_f32
            })
            .test_tag("yakın"));
        Text(::kompoz::runtime::kt::template!(
            "ölçek ",
            ::kompoz::runtime::kt::KtFloat::round_to_int((::kompoz::runtime::kt::KtDelegate::get_value(&scale) * 10.0)),
            " kayma ",
            ::kompoz::runtime::kt::KtFloat::round_to_int(::kompoz::runtime::kt::KtDelegate::get_value(&moved))
        ));
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Fruit(name: impl ::core::convert::Into<::std::string::String>, target: ::std::rc::Rc<dyn DragAndDropTarget>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Column(|| {
        Box(modifier = Modifier
            .size_wh(80.dp(), 40.dp())
            .drag_and_drop_source({
                let name = ::kompoz::runtime::__private::capture(&name);
                move |_| {
                    ::core::option::Option::Some(drag_and_drop_transfer_data(
                        text = name.clone(),
                        mime_types = ::kompoz::runtime::kt::list_of([::std::string::String::from("text/plain")]),
                    ))
                }
            })
            .test_tag("meyve"));
        Box(modifier =
            Modifier.size(200.dp()).drag_and_drop_target(move |it| it.has_mime_type("text/plain"), target.clone()));
    });
}
