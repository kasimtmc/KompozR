//! Resources in Kotlin's syntax: tests/composeResources' `Res` (`compose_resources!`) read by a `.kt` file
//! (kompoz_file!, kt/resources.kt) as Compose Multiplatform's users write it and as Android's do (`R.*`, `id =`,
//! `count =`), and by `kompoz! { … }`: strings in the UI's language (switched by LocalStrings), formatted, quantity
//! strings, arrays, a bitmap's and a vector's painters, their bitmap and vector, a font of a font resource, the dark
//! theme's drawable; AsyncImage of a resource file's URI (`Res.getUri`).

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::KompozTestRule;
use kompoz::ui::strings::{LocalStrings, Strings};

compose_resources!(dir = "tests/composeResources");

kompoz_file!("tests/kt/resources.kt");

kompoz! {
    @Composable
    fun InlineResources(dark: Boolean) {
        val image = imageResource(Res.drawable.badge)
        Text("${image.width}x${image.height} ${Res.allDrawableResources.size}", modifier = Modifier.testTag("inline"))
        AsyncImage(model = Res.getUri("files/hello.txt"), contentDescription = "dosya", modifier = Modifier.size(10.dp))
    }
}

fn text_of(rule: &KompozTestRule, tag: &str) -> String {
    let node = rule.on_node_with_tag(tag).fetch_semantics_node();
    node.config()
        .get(kompoz::ui::semantics::SemanticsProperties::Text)
        .map(|t| t[0].text().to_string())
        .unwrap_or_default()
}

fn screen(locale: MutableState<String>, count: MutableState<i32>) -> KompozTestRule {
    KompozTestRule::new(kompozable_lambda!(move || {
        let strings = Strings { locale: locale.get(), ..Strings::english() };
        let count = count.get();
        KompozitionLocalProvider(&[LocalStrings.provides(strings)], || {
            Column(
                content = || {
                    ResourcesScreen(count, "Ada".to_string());
                    AndroidResources(count);
                    InlineResources(false);
                },
            );
        });
    }))
}

/// kt/resources.kt's screen in English, then switched to Turkish and to another count.
fn resources_as_kotlin_writes_them() {
    let (locale, count) = (mutable_state_of("en-US".to_string()), mutable_state_of(1));
    let rule = screen(locale.clone(), count.clone());
    assert_eq!(text_of(&rule, "title"), "Resources");
    assert_eq!(text_of(&rule, "welcome"), "Welcome, Ada!");
    assert_eq!(text_of(&rule, "items"), "1 item");
    assert_eq!(text_of(&rule, "days"), "Monday, Tuesday");
    assert_eq!(text_of(&rule, "android title"), "Resources");
    assert_eq!(text_of(&rule, "android items"), "1 item");
    assert_eq!(text_of(&rule, "inline"), "12x6 7");
    for (description, w, h) in
        [("badge", 12.0, 6.0), ("bitmap", 12.0, 6.0), ("arrow", 24.0, 24.0), ("vector", 24.0, 24.0)]
    {
        let b = rule.on_node_with_content_description(description).get_unclipped_bounds_in_root();
        assert!((b.width() - w).abs() < 0.5 && (b.height() - h).abs() < 0.5, "{description}: {b:?}");
    }
    assert!(rule.on_node_with_content_description("Resources").exists()); // (Android's image's description)
    count.set(3);
    locale.set("tr".into());
    rule.wait_for_idle();
    assert_eq!(text_of(&rule, "title"), "Kaynaklar");
    assert_eq!(text_of(&rule, "welcome"), "Hoş geldin, Ada!");
    assert_eq!(text_of(&rule, "items"), "3 öğe");
    assert_eq!(text_of(&rule, "days"), "Pazartesi, Salı");
    assert_eq!(text_of(&rule, "android items"), "3 öğe");
    assert!(rule.on_node_with_content_description("Kaynaklar").exists());
    // The font: its family of the resource's bytes.
    assert!(rule.drawn_texts().iter().any(|t| t == "kalın"));
}

/// The dark theme's drawable where the system is dark (a red badge in the light theme, a blue one in the dark).
fn the_theme_picks_the_drawable() {
    let corner = |dark: bool| {
        let rule = KompozTestRule::empty();
        rule.host().set_system_dark_theme(dark);
        let seen = std::rc::Rc::new(std::cell::RefCell::new(None));
        let s = seen.clone();
        rule.set_content(kompozable_lambda!(move || {
            *s.borrow_mut() = Some(image_resource(Res::drawable::badge()));
        }));
        rule.wait_for_idle();
        let image: ImageBitmap = seen.borrow().clone().unwrap();
        let p = image.pixels();
        [p[0], p[1], p[2]]
    };
    assert_eq!(corner(false), [255, 0, 0]);
    assert_eq!(corner(true), [0, 0, 255]);
}

fn main() {
    common::run(&[
        ("resources_as_kotlin_writes_them", resources_as_kotlin_writes_them),
        ("the_theme_picks_the_drawable", the_theme_picks_the_drawable),
    ]);
}
