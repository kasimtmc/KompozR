#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Described(selected: impl ::core::convert::Into<::core::option::Option<i32>>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let title: ::std::string::String;
    let description: ::std::string::String;
    if let ::core::option::Option::Some(selected) = selected.clone() {
        title = ::kompoz::runtime::kt::template!("Item ", selected);
        description = ::kompoz::runtime::kt::template!("This is a description about ", title, ".");
    } else {
        title = ::std::string::String::from("No item selected");
        description = ::std::string::String::from("Select an item from the list.");
    }
    Text(title.clone());
    Text(description.clone());
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
#[doc(hidden)]
pub fn __kz_ext_Proportioned(
    __kz_this: ThreePaneScaffoldPaneScope,
    width: f32,
    content: impl ::core::ops::Fn() + 'static,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    __kz_this.AnimatedPane(
        modifier =
            __kz_this.preferred_height_proportion(__kz_this.preferred_width_proportion(Modifier, width), 0.5_f32),
        content = |this1| {
            let _ = &this1;
            content();
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
# [:: kompoz :: runtime :: named (pub trait ThreePaneScaffoldPaneScopeProportionedExt)]
impl ThreePaneScaffoldPaneScope {
    #[track_caller]
    fn Proportioned(&self, width: f32, content: impl ::core::ops::Fn() + 'static) {
        let __kz_receiver = ::core::clone::Clone::clone(self);
        ::kompoz::runtime::kompozable_block!({ __kz_ext_Proportioned(__kz_receiver, width = width, content = content) })
    }
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Routed(route: impl ::core::convert::Into<::std::string::String>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let holder = remember_saveable_state_holder();
    holder.SaveableStateProvider(&route.clone(), || {
        Text(route.clone());
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Compared(
    items: ::std::vec::Vec<::std::string::String>,
    selected: impl ::core::convert::Into<::core::option::Option<i32>>,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::kt::KtList::for_each_indexed(&items, |index, item| {
        Text(
            item.clone(),
            color = if ::core::option::Option::Some(index) == selected { Color::Red } else { Color::Black },
        );
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Picked(
    items: ::std::vec::Vec<::std::string::String>,
    picked: impl ::core::convert::Into<::core::option::Option<NavItemData>>,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    ::kompoz::runtime::kt::KtList::for_each_indexed(&items, |index, item| {
        Text(
            item.clone(),
            color = if ::core::option::Option::Some(index)
                == ::kompoz::runtime::kt::safe!(picked.clone(), |__kz_it1| __kz_it1.index())
            {
                Color::Red
            } else {
                Color::Black
            },
        );
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Routes(start: impl ::core::convert::Into<::std::string::String>) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let holder = remember_saveable_state_holder();
    AnimatedContent(start.clone(), |this, route| {
        let _ = &this;
        holder.SaveableStateProvider(&route.clone(), || {
            Text(route.clone());
        });
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
fn RadioRow(
    selected: bool,
    on_click: impl ::core::ops::Fn() + 'static,
    text: impl ::core::convert::Into<::std::string::String>,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Row(
        Modifier.selectable(
            selected = selected,
            on_click = on_click,
            role = ::core::option::Option::Some(Role::RadioButton),
        ),
        content = || {
            RadioButton(
                selected,
                ::core::option::Option::Some({
                    let __kz_f2: ::std::rc::Rc<dyn ::core::ops::Fn()> = ::std::rc::Rc::new(on_click.clone());
                    __kz_f2
                }),
            );
            Text(text.clone());
        },
    );
}
::std::thread_local! { # [allow (non_snake_case , non_upper_case_globals , unused_parens , unused_braces , unused_mut , while_true , clippy :: all)] static Anchors : :: std :: vec :: Vec < PaneExpansionAnchor > = { # [allow (unused_imports)] use :: kompoz :: runtime :: kt :: ext :: * ; :: kompoz :: runtime :: named_args ! (:: kompoz :: runtime :: kt :: list_of ([PaneExpansionAnchor :: Proportion (0_f32) , :: core :: convert :: Into :: < PaneExpansionAnchor > :: into (PaneExpansionAnchorOffset :: from_start (240 . dp ())) , PaneExpansionAnchor :: Proportion (1_f32)])) } ; }
