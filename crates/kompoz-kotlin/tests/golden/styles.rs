#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Container(
    enabled: bool,
    shape: ::std::sync::Arc<dyn Shape>,
    #[default(::std::rc::Rc::new(Style))] style: ::std::rc::Rc<dyn Style>,
) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let interaction_source = remember(|| MutableInteractionSource::new());
    let style_state = remember_keyed(interaction_source.clone(), || {
        MutableStyleState::new(::core::option::Option::Some(::core::convert::Into::<InteractionSource>::into(
            interaction_source.clone(),
        )))
    });
    Box(Modifier
        .styleable(
            ::core::option::Option::Some(::core::convert::Into::<StyleState>::into(style_state.clone())),
            style = {
                let shape = ::kompoz::runtime::__private::capture(&shape);
                move |this: &dyn StyleScope| {
                    this.shape(shape.clone());
                    this.background(Color::White);
                    this.size(10.dp());
                    this.translation_x(this.to_px(2.dp()));
                    this.focused(|| {
                        this.animate_spec(::std::sync::Arc::new(tween(duration_millis = 100)), || {
                            this.background(Color::Blue);
                        });
                    });
                    if this.state().get(&PlayingKey.with(::core::clone::Clone::clone)) {
                        this.alpha(0.5_f32);
                    }
                }
            },
        )
        .styleable(::core::option::Option::None, style = style.clone()));
}
::std::thread_local! { # [allow (non_snake_case , non_upper_case_globals , unused_parens , unused_braces , unused_mut , while_true , clippy :: all)] pub static PlayingKey : StyleStateKey < bool > = { # [allow (unused_imports)] use :: kompoz :: runtime :: kt :: ext :: * ; :: kompoz :: runtime :: named_args ! (StyleStateKey :: new (false)) } ; }
::std::thread_local! { # [allow (non_snake_case , non_upper_case_globals , unused_parens , unused_braces , unused_mut , while_true , clippy :: all)] pub static Base : :: std :: rc :: Rc < dyn Style > = { # [allow (unused_imports)] use :: kompoz :: runtime :: kt :: ext :: * ; :: kompoz :: runtime :: named_args ! ({ let __kz_f1 : :: std :: rc :: Rc < dyn Style > = :: std :: rc :: Rc :: new (move | this : & dyn StyleScope | { this . background (Color :: Red) ; }) ; __kz_f1 }) } ; }
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn Uses(playing: bool) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let state = remember_updated_style_state(
        ::core::option::Option::None,
        block = |it| {
            it.set(&PlayingKey.with(::core::clone::Clone::clone), playing);
        },
    );
    Container(
        enabled = true,
        shape = ::std::sync::Arc::new(RectangleShape),
        style = {
            let __kz_f2: ::std::rc::Rc<dyn Style> = ::std::rc::Rc::new(move |this: &dyn StyleScope| {
                this.alpha(0.5_f32);
            });
            __kz_f2
        },
    );
    Box(Modifier.styleable(
        ::core::option::Option::Some(state.clone()),
        style = combined_style(Base.with(::core::clone::Clone::clone), move |this: &dyn StyleScope| {
            this.size(4.dp());
        }),
    ));
    Box(Modifier.styleable(
        ::core::option::Option::Some(state.clone()),
        style = Base.with(::core::clone::Clone::clone).then(Style),
    ));
}
