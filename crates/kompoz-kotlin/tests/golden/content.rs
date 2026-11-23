#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn NestedReceivers(log: impl ::core::ops::Fn(::std::string::String) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Box(
        modifier = Modifier.size(200.dp()).content_receiver({
            let log = ::kompoz::runtime::__private::capture(&log);
            move |content: TransferableContent| {
                log(::kompoz::runtime::kt::template!("dış ", (content.clip_entry().item_count() as i32)));
                content.consume(|item| item.text().is_some())
            }
        }),
        content = || {
            Box(modifier = Modifier.size(100.dp()).content_receiver({
                let log = ::kompoz::runtime::__private::capture(&log);
                move |content: TransferableContent| {
                    log(::kompoz::runtime::kt::template!(
                        "iç ",
                        (content.clip_entry().item_count() as i32),
                        " ",
                        content.has_media_type(&MediaType::Image)
                    ));
                    content.consume(|item| item.has_mime_type("image/*"))
                }
            }));
        },
    );
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn ClipboardButtons() {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let clipboard = LocalClipboard.current();
    let scope = remember_coroutine_scope();
    let pasted = remember(|| ::kompoz::runtime::mutable_state_of!(::std::string::String::from("")));
    Column(|| {
        Text(
            "kopyala",
            modifier = Modifier.clickable({
                let scope = ::kompoz::runtime::__private::capture(&scope);
                let clipboard = ::kompoz::runtime::__private::capture(&clipboard);
                move || {
                    scope.launch({
                        let clipboard = ::kompoz::runtime::__private::capture(&clipboard);
                        async move {
                            clipboard
                                .set_clip_entry(::core::option::Option::Some(ClipEntry::with_plain_text("panodan")))
                                .await;
                        }
                    });
                }
            }),
        );
        Text(
            "yapıştır",
            modifier = Modifier.clickable({
                let scope = ::kompoz::runtime::__private::capture(&scope);
                let pasted = ::kompoz::runtime::__private::capture(&pasted);
                let clipboard = ::kompoz::runtime::__private::capture(&clipboard);
                move || {
                    scope.launch({
                        let pasted = ::kompoz::runtime::__private::capture(&pasted);
                        let clipboard = ::kompoz::runtime::__private::capture(&clipboard);
                        async move {
                            ::kompoz::runtime::kt::KtMutableDelegate::set_value(
                                &pasted,
                                ::kompoz::runtime::kt::elvis(
                                    ::kompoz::runtime::kt::safe!(clipboard.get_clip_entry().await, |__kz_it1| __kz_it1
                                        .read_plain_text()),
                                    || ::std::string::String::from(""),
                                ),
                            );
                        }
                    });
                }
            }),
        );
        Text(::kompoz::runtime::kt::template!("pano: ", ::kompoz::runtime::kt::KtDelegate::get_value(&pasted)));
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn PasteImages(received: impl ::core::ops::Fn(i32, i32) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    let state = remember_text_field_state();
    Column(|| {
        BasicTextField(
            state = state.clone(),
            modifier = Modifier.width(300.dp()).test_tag("alan").content_receiver({
                let received = ::kompoz::runtime::__private::capture(&received);
                move |transferable_content: TransferableContent| {
                    if !transferable_content.has_media_type(&MediaType::Image) {
                        return ::core::option::Option::Some(transferable_content.clone());
                    }
                    transferable_content.consume(|item| {
                        let image = item.image();
                        if let ::core::option::Option::Some(image) = image.clone() {
                            received(image.width(), image.height());
                            true
                        } else {
                            false
                        }
                    })
                }
            }),
        );
        Text(::kompoz::runtime::kt::template!("alan: ", state.text()));
    });
}
#[allow(non_snake_case, non_upper_case_globals, unused_parens, unused_braces, unused_mut, while_true, clippy::all)]
#[::kompoz::runtime::kompozable]
pub fn DropFiles(dropped: impl ::core::ops::Fn(::std::string::String) + 'static) {
    #[allow(unused_imports)]
    use ::kompoz::runtime::kt::ext::*;
    Box(modifier = Modifier.size(100.dp()).content_receiver({
        let dropped = ::kompoz::runtime::__private::capture(&dropped);
        move |content: TransferableContent| {
            if content.source() != TransferableContentSource::DragAndDrop {
                ::core::option::Option::Some(content.clone())
            } else {
                content.consume(|item| {
                    ::kompoz::runtime::kt::KtList::for_each(&item.files(), |it| {
                        dropped(it.clone());
                    });
                    ::kompoz::runtime::kt::KtList::is_not_empty(&item.files())
                })
            }
        }
    }));
}
