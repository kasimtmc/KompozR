//! Material's basic components as Compose's material3 has them: Text and ProvideTextStyle, Icon, Surface and its
//! clickable kinds, the ripple and its configuration, dividers, the scrim, the minimum interactive size. The C++
//! Kompoz's tests by name (kompoz_tests.cpp: their Material halves where the rest is a lower crate's), and Compose's
//! behaviour.

#![allow(non_snake_case, non_upper_case_globals)]

#[path = "../../kompoz-foundation/tests/common/mod.rs"]
mod common;

use common::*;
use kompoz_foundation::prelude::*;
use kompoz_graphics::*;
use kompoz_material_icons_core::Icons;
use kompoz_material_ripple::RippleAlpha;
use kompoz_material3::prelude::*;
use kompoz_runtime::*;
use kompoz_test::KompozTestRule;
use kompoz_text::{AnnotatedString, FontWeight, LinkAnnotation, TextDecoration, TextStyle};
use kompoz_ui::draw::{BitmapPainter, ColorPainter, Painter};
use kompoz_ui::input::PointerType;
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::semantics::{Role, SemanticsProperties};
use kompoz_ui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

tests!(
    minimum_interactive_component_size_keeps_room_to_touch,
    icon_sizes_tints_and_draws,
    painters_size_and_draw_images_icons_and_paint,
    provide_text_style_and_text_without_a_direction_follow_the_layout,
    text_sizes_in_em_are_shares_of_a_font_size,
    font_feature_settings_reach_the_fonts_text_is_drawn_in,
    text_takes_the_theme_and_the_content_color,
    text_links_take_the_primary_color,
    surfaces_draw_their_shape_and_give_a_content_color,
    clickable_surfaces_click_select_and_toggle,
    ripples_take_the_content_color_and_the_configuration,
    dividers_are_lines_of_their_thickness,
    scrims_cover_and_dismiss,
);

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

fn minimum_interactive_component_size_keeps_room_to_touch() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Box(
                modifier = Modifier.test_tag("dış").minimum_interactive_component_size(),
                content = || {
                    Box(modifier = Modifier.size(20.dp()).test_tag("iç"));
                },
            );
            Box(
                modifier = Modifier.test_tag("büyük").minimum_interactive_component_size(),
                content = || {
                    Box(modifier = Modifier.size_wh(60.dp(), 30.dp()));
                },
            );
            KompozitionLocalProvider(&[LocalMinimumInteractiveComponentSize.provides(Dp(0.0))], || {
                Box(
                    modifier = Modifier.test_tag("kapalı").minimum_interactive_component_size(),
                    content = || {
                        Box(modifier = Modifier.size(20.dp()));
                    },
                );
            });
        });
    }));
    let (outer, inner) = (bounds(&rule, "dış"), bounds(&rule, "iç"));
    // (centred in it)
    assert!(outer.width() == 48.0 && outer.height() == 48.0);
    assert!(inner.left == outer.left + 14.0 && inner.top == outer.top + 14.0);
    // (only what is smaller grows)
    let big = bounds(&rule, "büyük");
    assert!(big.width() == 60.0 && big.height() == 48.0);
    assert_eq!(bounds(&rule, "kapalı").width(), 20.0);
    // (where it was put, told as Compose's alignment lines)
    let node = rule.on_node_with_tag("dış").fetch_semantics_node();
    let _ = node;
}

fn icon_sizes_tints_and_draws() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        KompozitionLocalProvider(&[LocalContentColor.provides(Color::hex(0x123456))], || {
            Row(|| {
                VectorIcon(Icons::Filled::Add(), Some("Ekle".into()), modifier = Modifier.test_tag("small"));
                VectorIcon(
                    Icons::Filled::Star(),
                    Some("Yıldız".into()),
                    modifier = Modifier.size(48.dp()).test_tag("big"),
                );
            });
        });
    }));
    assert!(near(bounds(&rule, "small").width(), 24.0, 0.01));
    assert!(near(bounds(&rule, "big").width(), 48.0, 0.01));
    // (the description is its semantics', an image, as Compose's)
    let described = rule.on_node_with_content_description("Ekle").fetch_semantics_node();
    assert_eq!(described.config().get(SemanticsProperties::Role), Some(Role::Image));
    let canvas = rule.draw();
    assert_eq!(canvas.vectors, vec![Icons::Filled::Add().id(), Icons::Filled::Star().id()]);
    // (tinted the content color)
    assert!(canvas.vector_colors.iter().all(|c| *c == Color::hex(0x123456)), "{:?}", canvas.vector_colors);
}

fn painters_size_and_draw_images_icons_and_paint() {
    // The C++ test's Icon half (its painters and Image: kompoz-ui's draw tests): a painter with no size of its own is
    // 24 dp, tinted the content color; a bitmap's icon its pixels' size.
    let wide = ImageBitmap::from_pixels(40, 20, vec![255; 40 * 20 * 4]).expect("40 by 20 pixels");
    let content = light_color_scheme().on_surface;
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let wide = wide.clone();
        KompozitionLocalProvider(&[LocalContentColor.provides(content)], move || {
            let wide = wide.clone();
            Column(move || {
                let red: Rc<dyn Painter> = Rc::new(ColorPainter(Color::new(1.0, 0.0, 0.0, 1.0)));
                Icon(red, Some("renk".into()), modifier = Modifier.test_tag("renk"));
                let bitmap: Rc<dyn Painter> = Rc::new(BitmapPainter::with_quality(wide.clone(), DefaultFilterQuality));
                Icon(bitmap, Some("resim".into()), modifier = Modifier.test_tag("resim"));
                BitmapIcon(wide.clone(), None, modifier = Modifier.test_tag("bitmap"), tint = Color::Unspecified);
            });
        });
    }));
    let size = |tag: &str| {
        let b = bounds(&rule, tag);
        (b.width(), b.height())
    };
    assert_eq!((size("renk"), size("resim"), size("bitmap")), ((24.0, 24.0), (40.0, 20.0), (40.0, 20.0)));
    let canvas = rule.draw();
    let at = bounds(&rule, "renk");
    let tinted = canvas
        .rects
        .iter()
        .zip(&canvas.rect_colors)
        .any(|(r, c)| near(r.left, at.left, 0.5) && near(r.width(), 24.0, 0.5) && *c == content);
    assert!(tinted, "{:?} {:?}", canvas.rects, canvas.rect_colors);
}

fn provide_text_style_and_text_without_a_direction_follow_the_layout() {
    // The C++ test's Material half (its text field is the text fields' work): ProvideTextStyle merges into the style
    // around; a text of digits starts at the right right to left.
    let rtl = mutable_state_of(true);
    let r = rtl.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let r = r.clone();
        Column(move || {
            let style = TextStyle { font_size: 20.sp(), color: Color::hex(0x336699), ..TextStyle::Default };
            ProvideTextStyle(style, || {
                Text("Başlık");
                // (merged: still 20)
                ProvideTextStyle(TextStyle { font_weight: Some(FontWeight::Bold), ..TextStyle::Default }, || {
                    Text("Kalın")
                });
            });
            Text("Düz");
            let direction = if r.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
            KompozitionLocalProvider(&[LocalLayoutDirection.provides(direction)], || {
                Text("12345", modifier = Modifier.width(200.dp()).test_tag("rakam"));
                Text("Merhaba", modifier = Modifier.width(200.dp()).test_tag("latin"));
            });
        });
    }));
    let canvas = rule.draw();
    let at = |t: &str| canvas.texts.iter().position(|x| x == t).unwrap();
    let (title, bold, plain) = (at("Başlık"), at("Kalın"), at("Düz"));
    assert!(near(canvas.text_styles[title].font_size, 20.0, 0.01) && canvas.colors[title] == Color::hex(0x336699));
    assert!(
        near(canvas.text_styles[bold].font_size, 20.0, 0.01)
            && canvas.text_styles[bold].font_weight == FontWeight::Bold
    );
    assert_eq!(canvas.colors[bold], Color::hex(0x336699));
    assert!(!near(canvas.text_styles[plain].font_size, 20.0, 0.01) && canvas.colors[plain] != Color::hex(0x336699));
    // A text of no direction of its own takes the layout's (Compose's resolveTextDirection: Unspecified is Rtl right to
    // left, not ContentOrRtl): digits and Latin letters alike start at the right.
    let (digits, latin) = (bounds(&rule, "rakam"), bounds(&rule, "latin"));
    assert!(canvas.baselines[at("12345")].x > digits.left + 100.0);
    assert!(canvas.baselines[at("Merhaba")].x > latin.left + 100.0);
    rtl.set(false);
    rule.wait_for_idle();
    let ltr = rule.draw();
    let i = ltr.texts.iter().position(|x| x == "12345").unwrap();
    assert!(near(ltr.baselines[i].x, digits.left, 0.5));
}

fn text_sizes_in_em_are_shares_of_a_font_size() {
    // The C++ test's ProvideTextStyle part (the rest: kompoz-foundation's text tests): a letter spacing in em of the
    // text's own size, a line height in em. A text's own font size in em replaces the style's around (Text's merge)
    // and is a share of the default text size (14 sp), as Compose's paragraph resolves a whole text's em size — not
    // of the 20 sp around, as the C++ took it.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            ProvideTextStyle(TextStyle { font_size: 20.sp(), ..TextStyle::Default }, || {
                // (of the default 14; of its own 20)
                Text("büyük", font_size = 1.5.em());
                Text("aralıklı", letter_spacing = 0.1.em());
            });
            Text("satır", font_size = 10.sp(), line_height = 1.5.em());
        });
    }));
    let canvas = rule.draw();
    let style = |t: &str| canvas.text_styles[canvas.texts.iter().position(|x| x == t).unwrap()].clone();
    assert!(near(style("büyük").font_size, 21.0, 0.01) && near(style("aralıklı").font_size, 20.0, 0.01));
    assert!(near(style("aralıklı").letter_spacing, 2.0, 0.01));
    assert!(near(style("satır").line_height.unwrap_or(0.0), 15.0, 0.01));
}

fn font_feature_settings_reach_the_fonts_text_is_drawn_in() {
    // The C++ test's Text and ProvideTextStyle parts: a style's feature settings, the style around's.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Text("123", style = TextStyle { font_feature_settings: Some("tnum".into()), ..TextStyle::Default });
            ProvideTextStyle(TextStyle { font_feature_settings: Some("frac".into()), ..TextStyle::Default }, || {
                Text("1/2")
            });
            Text("ab");
        });
    }));
    let canvas = rule.draw();
    let features =
        |t: &str| canvas.text_styles[canvas.texts.iter().position(|x| x == t).unwrap()].font_feature_settings.clone();
    assert_eq!((features("123"), features("1/2"), features("ab")), ("tnum".into(), "frac".into(), String::new()));
}

fn text_takes_the_theme_and_the_content_color() {
    // Compose's Text: 14 sp black outside a theme; MaterialTheme's bodyLarge (16 sp in 24) in the content color
    // inside one; `color` over the style's over the content color.
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        Column(|| {
            Text("dışarıda", modifier = Modifier.test_tag("dış"));
            MaterialTheme(
                content = || {
                    Text("içeride", modifier = Modifier.test_tag("iç"));
                    KompozitionLocalProvider(&[LocalContentColor.provides(Color::hex(0x00AA00))], || {
                        Text("yeşil");
                        Text("kırmızı", color = Color::Red);
                        Text(
                            "üslu",
                            style = TextStyle { color: Color::Blue, ..MaterialTheme::typography().label_small },
                        );
                    });
                },
            );
        });
    }));
    let canvas = rule.draw();
    let at = |t: &str| canvas.texts.iter().position(|x| x == t).unwrap();
    assert!(
        near(canvas.text_styles[at("dışarıda")].font_size, 14.0, 0.01) && canvas.colors[at("dışarıda")] == Color::Black
    );
    assert!(near(canvas.text_styles[at("içeride")].font_size, 16.0, 0.01));
    assert_eq!(bounds(&rule, "iç").height(), 24.0);
    assert_eq!(canvas.colors[at("yeşil")], Color::hex(0x00AA00));
    assert_eq!(canvas.colors[at("kırmızı")], Color::Red);
    assert_eq!(canvas.colors[at("üslu")], Color::Blue);
    assert!(near(canvas.text_styles[at("üslu")].font_size, 11.0, 0.01));
}

fn text_links_take_the_primary_color() {
    // Compose's Text of an annotated string: a link without styles of its own in the theme's primary, underlined.
    let text = {
        let mut b = kompoz_text::AnnotatedStringBuilder::new();
        b.append("bak: ");
        b.push_link(LinkAnnotation::url("https://example.org"));
        b.append("bağlantı");
        b.pop();
        b.to_annotated_string()
    };
    let seen: Rc<RefCell<Option<AnnotatedString>>> = Rc::default();
    let s = seen.clone();
    let _rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (text, s) = (text.clone(), s.clone());
        MaterialTheme(
            color_scheme = dark_color_scheme(),
            content = move || {
                let s = s.clone();
                Text(
                    text.clone(),
                    on_text_layout = Some(Rc::new(move |l| *s.borrow_mut() = Some(l.layout_input.text.clone()))),
                );
            },
        );
    }));
    let laid_out = seen.borrow().clone().expect("laid out");
    let links = laid_out.get_link_annotations(0, laid_out.text().len());
    let styles = links[0].item.styles().and_then(|s| s.style.clone()).expect("the link's styles");
    assert_eq!(styles.color, dark_color_scheme().primary);
    assert_eq!(styles.text_decoration, Some(TextDecoration::Underline));
}

fn surfaces_draw_their_shape_and_give_a_content_color() {
    // Compose's Surface: its color in its shape (cut to it), a border, a shadow of its elevation; its content in the
    // color's content color, at least its min constraints; it takes the pointer, a container to accessibility.
    let content_seen = Rc::new(Cell::new(Color::Unspecified));
    let c = content_seen.clone();
    let behind = Rc::new(Cell::new(0));
    let b = behind.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (c, b) = (c.clone(), b.clone());
        MaterialTheme(
            content = move || {
                let (c, b) = (c.clone(), b.clone());
                Box(move || {
                    let b = b.clone();
                    Box(modifier = Modifier.size(100.dp()).clickable(move || b.set(b.get() + 1)));
                    let c = c.clone();
                    Surface(
                        modifier = Modifier.size(80.dp()).test_tag("yüzey"),
                        shape = Arc::new(RoundedCornerShape(8.dp())),
                        color = MaterialTheme::color_scheme().primary,
                        shadow_elevation = Dp(4.0),
                        border = Some(BorderStroke(1.dp(), Color::Red)),
                        content = move || {
                            c.set(LocalContentColor.current());
                            Box(modifier = Modifier.size(10.dp()).test_tag("içerik"));
                        },
                    );
                });
            },
        );
    }));
    assert_eq!(content_seen.get(), light_color_scheme().on_primary);
    // (Box's min constraints: the content fills the surface)
    assert_eq!(bounds(&rule, "içerik").width(), 80.0);
    let canvas = rule.draw();
    assert!(
        canvas.path_colors.contains(&light_color_scheme().primary)
            || canvas.rect_colors.contains(&light_color_scheme().primary)
    );
    assert!(!canvas.clips.is_empty() || !canvas.clip_shapes.is_empty() || !canvas.layer_bounds.is_empty());
    // (it takes the pointer: what is under it isn't clicked)
    let at = bounds(&rule, "yüzey").center();
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    rule.host().pointer_up(at, -1, 0);
    rule.wait_for_idle();
    assert_eq!(behind.get(), 0);
    let node = rule.on_node_with_tag("yüzey").fetch_semantics_node();
    assert_eq!(node.config().get(SemanticsProperties::IsContainer), Some(true));
}

fn clickable_surfaces_click_select_and_toggle() {
    // Compose's Surface(onClick), Surface(selected, onClick), Surface(checked, onCheckedChange): at least 48 dp, the
    // ripple, a click each, their state to accessibility, the content's semantics theirs.
    let clicks = Rc::new(Cell::new(0));
    let checked = mutable_state_of(false);
    let (c, ch) = (clicks.clone(), checked.clone());
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let (c, ch) = (c.clone(), ch.clone());
        MaterialTheme(
            content = move || {
                let (c, ch) = (c.clone(), ch.clone());
                Column(move || {
                    let c = c.clone();
                    // (the room it takes: the Box around it, its own semantics bounds the clickable's, as Compose's)
                    Box(
                        modifier = Modifier.test_tag("tık kutusu"),
                        content = move || {
                            let c = c.clone();
                            ClickableSurface(
                                move || c.set(c.get() + 1),
                                modifier = Modifier.test_tag("tık"),
                                content = || {
                                    Text("tıkla");
                                },
                            );
                        },
                    );
                    SelectableSurface(true, || {}, modifier = Modifier.test_tag("seçili"), content = || Text("seç"));
                    let ch2 = ch.clone();
                    ToggleableSurface(
                        ch.get(),
                        move |v| ch2.set(v),
                        modifier = Modifier.test_tag("aç"),
                        content = || Text("aç kapa"),
                    );
                    Box(
                        modifier = Modifier.test_tag("kapalı kutusu"),
                        content = || {
                            ClickableSurface(
                                || {},
                                enabled = false,
                                modifier = Modifier.test_tag("kapalı"),
                                content = || {},
                            );
                        },
                    );
                });
            },
        );
    }));
    // (the minimum interactive size)
    assert!(bounds(&rule, "tık kutusu").height() >= 48.0 && bounds(&rule, "kapalı kutusu").width() >= 48.0);
    rule.on_node_with_tag("tık").perform_click();
    assert_eq!(clicks.get(), 1);
    // (the content's text is the clickable's)
    rule.on_node_with_tag("tık").assert_text_equals(&["tıkla"]);
    rule.on_node_with_tag("seçili").assert_is_selected();
    rule.on_node_with_tag("aç").assert_is_off().perform_click();
    assert!(checked.get());
    rule.on_node_with_tag("aç").assert_is_on();
    rule.on_node_with_tag("kapalı").assert_is_not_enabled();
}

fn ripples_take_the_content_color_and_the_configuration() {
    // material3's ripple: MaterialTheme's indication, in the content color at Material's alphas; a configuration's
    // color and alphas where provided, none where it provides none.
    assert_eq!(RippleDefaults::RippleAlpha, RippleAlpha::new(0.16, 0.1, 0.08, 0.1));
    assert_eq!(
        format!("{}", RippleConfiguration::default()),
        "RippleConfiguration(color=Color(0.0, 0.0, 0.0, 0.0, None), rippleAlpha=null)"
    );
    assert!(*ripple() == *ripple() && *ripple() != *named_args!(ripple(bounded = false)));
    let configuration = mutable_state_of(0);
    let cf = configuration.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let cf = cf.clone();
        MaterialTheme(
            content = move || {
                let config = match cf.get() {
                    0 => Some(RippleConfiguration::default()),
                    1 => Some(RippleConfiguration::new(Color::Red, Some(RippleAlpha::new(0.5, 0.5, 0.5, 0.5)))),
                    _ => None,
                };
                KompozitionLocalProvider(
                    &[LocalContentColor.provides(Color::Blue), LocalRippleConfiguration.provides(config)],
                    || {
                        Box(modifier = Modifier.size(40.dp()).test_tag("dokun").clickable(|| {}));
                    },
                );
            },
        );
    }));
    let pressed_color = |rule: &KompozTestRule| {
        let at = bounds(rule, "dokun").center();
        rule.host().pointer_down(at, -1, 0, PointerType::Touch);
        rule.advance_time_by(100);
        let canvas = rule.draw();
        rule.host().pointer_up(at, -1, 0);
        rule.advance_time_by(1000);
        // (the ripple's circle a round rect, its state layer a rect or a path)
        canvas.rect_colors.iter().chain(&canvas.path_colors).copied().find(|c| c.alpha() > 0.0 && c.alpha() < 1.0)
    };
    let blue = pressed_color(&rule).expect("a ripple");
    assert!(blue.blue() == 1.0 && blue.red() == 0.0 && near(blue.alpha(), 0.1, 0.06), "{blue:?}");
    configuration.set(1);
    rule.wait_for_idle();
    let red = pressed_color(&rule).expect("a ripple");
    assert!(red.red() == 1.0 && red.blue() == 0.0 && red.alpha() > 0.15, "{red:?}");
    configuration.set(2);
    rule.wait_for_idle();
    assert!(pressed_color(&rule).is_none());
}

fn dividers_are_lines_of_their_thickness() {
    let rule = KompozTestRule::new(kompozable_lambda!(|| {
        MaterialTheme(
            content = || {
                Column(
                    modifier = Modifier.width(100.dp()),
                    content = || {
                        HorizontalDivider(modifier = Modifier.test_tag("yatay"));
                        Row(
                            modifier = Modifier.height(50.dp()),
                            content = || {
                                VerticalDivider(modifier = Modifier.test_tag("dikey"), thickness = 3.dp());
                            },
                        );
                    },
                );
            },
        );
    }));
    let h = bounds(&rule, "yatay");
    assert_eq!((h.width(), h.height()), (100.0, 1.0));
    let v = bounds(&rule, "dikey");
    assert_eq!((v.width(), v.height()), (3.0, 50.0));
    let canvas = rule.draw();
    // (a line of a width drawn as a stroked path: the paths recorded)
    assert!(
        canvas.path_colors.iter().all(|c| *c == light_color_scheme().outline_variant) && canvas.path_colors.len() == 2
    );
    assert_eq!(DividerDefaults::Thickness, Dp(1.0));
}

fn scrims_cover_and_dismiss() {
    // Compose's Scrim: all the room, its color at its alpha; a tap and accessibility's click call onClick; none: no
    // clicks; an unspecified color draws nothing.
    let dismissed = Rc::new(Cell::new(0));
    let d = dismissed.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || {
        let d = d.clone();
        Box(
            modifier = Modifier.size(100.dp()),
            content = move || {
                let d = d.clone();
                Scrim(
                    Some("kapat".into()),
                    modifier = Modifier.test_tag("perde"),
                    on_click = Some(Rc::new(move || d.set(d.get() + 1))),
                    alpha = Rc::new(|| 2.0),
                    color = Color::Red,
                );
            },
        );
    }));
    assert_eq!(bounds(&rule, "perde").width(), 100.0);
    rule.on_node_with_content_description("kapat").perform_click();
    assert_eq!(dismissed.get(), 1);
    let at = bounds(&rule, "perde").center();
    rule.host().pointer_down(at, -1, 0, PointerType::Touch);
    rule.host().pointer_up(at, -1, 0);
    rule.wait_for_idle();
    assert_eq!(dismissed.get(), 2);
    // (alpha kept within 1)
    let canvas = rule.draw();
    assert!(canvas.rect_colors.contains(&Color::Red));
    let quiet = KompozTestRule::new(kompozable_lambda!(|| Scrim(None, color = Color::Unspecified)));
    assert!(quiet.draw().rect_colors.is_empty());
}
