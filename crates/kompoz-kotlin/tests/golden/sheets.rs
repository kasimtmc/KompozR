#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Sheets() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let scaffold_state = remember_bottom_sheet_scaffold_state();
    BottomSheetScaffold(
        sheet_content = || {
            Button(
                modifier = Modifier.focus_properties({
                    let scaffold_state = ::kompoz::runtime::__private::capture(&scaffold_state);
                    move |this| {
                        this.set_can_focus(scaffold_state.bottom_sheet_state().current_value() == SheetValue::Expanded);
                    }
                }),
                on_click = || {},
                content = || {
                    Text("Collapse");
                },
            );
        },
        scaffold_state = ::core::option::Option::Some(scaffold_state.clone()),
        sheet_drag_handle = ::core::option::Option::None,
        sheet_shape = ::std::sync::Arc::new(RoundedCornerShape::new(0.dp(), 24.dp(), 24.dp(), 0.dp())),
        content = |padding| {
            Box(Modifier.padding_values(padding.clone()));
        },
    );
}
