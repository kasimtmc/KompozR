//! Foundation's styles in Kotlin's syntax, as a Compose user writes them: Compose 1.12's style samples
//! (SimpleStyleSample, StyleStateSample, StyleAnimationSample, StyleForegroundBackgroundSample, StyleStateKeySample,
//! TextStyleTextMotionSample: `Style { … }` blocks with their receiver `StyleScope`, property setters, `pressed { }`,
//! `hovered { }`, `disabled { }`, `animate { }`, `rememberUpdatedStyleState(interactionSource) { it.isEnabled = … }`,
//! `Modifier.styleable(styleState, style)`, a custom `StyleStateKey`), material3 1.5's text field container
//! (`remember(interactionSource) { MutableStyleState(interactionSource) }`, `styleable(styleState) { shape(…);
//! background(…); focused { animate(spec) { background(…); border(…) } } }`), styles combined (`Style(a, b)`,
//! `a then b`, the empty `Style`), composition locals read in a style (`LocalX.currentValue`) and the state read
//! (`state.isPressed`); and in a `.kt` file (kompoz_file!) the same. The behaviour crates/kompoz-foundation/tests/
//! style.rs checks in Rust.

#![allow(non_snake_case, non_upper_case_globals)]

mod common;

use kompoz::prelude::*;
use kompoz::test::{KompozTestRule, RecordingCanvas};

fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
    rule.on_node_with_tag(tag).get_unclipped_bounds_in_root()
}

/// The colors of the rects drawn, in order.
fn rect_colors(c: &RecordingCanvas) -> Vec<Color> {
    c.rect_colors.clone()
}

/// The color the style's background is drawn in (the first rect: a clickable's indication draws over it).
fn background(rule: &KompozTestRule) -> Color {
    rule.draw().rect_colors[0]
}

fn down(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_down(p, -1, 0, kompoz::ui::input::PointerType::Touch);
}

fn up(rule: &KompozTestRule, p: Offset) {
    rule.host().pointer_up(p, -1, 0);
}

// ---- Compose's samples ------------------------------------------------------------------------------------------

kompoz! {
    // (SimpleStyleSample: a styleable box styled 150x150 green)
    @Composable
    fun StyleableBox(modifier: Modifier = Modifier, style: Style = Style) {
        Box(modifier = modifier.styleable(null, style))
    }

    @Composable
    fun SimpleStyle() {
        StyleableBox(
            modifier = Modifier.testTag("yeşil"),
            style = {
                background(Color.Green)
                size(150.dp)
            }
        )
    }

    // (StyleStateSample and StyleAnimationSample: a clickable styleable box)
    @Composable
    fun ClickableStyleableBox(
        onClick: () -> Unit,
        modifier: Modifier = Modifier,
        style: Style = Style,
        enabled: Boolean = true,
    ) {
        val interactionSource = remember { MutableInteractionSource() }
        val styleState = rememberUpdatedStyleState(interactionSource) { it.isEnabled = enabled }
        Box(
            modifier =
                modifier
                    .clickable(interactionSource = interactionSource, onClick = onClick)
                    .styleable(styleState, style)
        )
    }

    @Composable
    fun StyleStates(enabled: Boolean) {
        ClickableStyleableBox(
            onClick = {},
            modifier = Modifier.testTag("durumlu"),
            enabled = enabled,
            style = {
                background(Color.Green)
                size(150.dp)
                hovered { background(Color.Yellow) }
                pressed { background(Color.Red) }
                disabled { background(Color.Gray) }
            },
        )
    }

    @Composable
    fun StyleAnimations() {
        ClickableStyleableBox(
            onClick = {},
            modifier = Modifier.testTag("canlı"),
            style = {
                background(Color.Blue)
                size(150.dp)
                hovered { animate { background(Color.Yellow) } }
                pressed { animate(tween(100, easing = LinearEasing)) { background(Color.Red) } }
                disabled { animate { background(Color.Gray) } }
            },
        )
    }

    // (StyleForegroundBackgroundSample: a semi-transparent overlay when pressed)
    @Composable
    fun StyleForegroundBackground() {
        val interactionSource = remember { MutableInteractionSource() }
        val styleState = rememberUpdatedStyleState(interactionSource)
        Box(
            modifier =
                Modifier
                    .clickable(interactionSource = interactionSource, onClick = {})
                    .styleable(styleState, {
                        size(150.dp)
                        background(Color.Blue)
                        pressed { foreground(Color.Black.copy(alpha = 0.4f)) }
                    })
                    .testTag("örtülü")
        ) {
            BasicText("Hello")
        }
    }

    // (TextStyleTextMotionSample: a style made with `Style { }`, a font size animated when pressed)
    @Composable
    fun TextMotionStyle() {
        val interactionSource = remember { MutableInteractionSource() }
        val styleState = rememberUpdatedStyleState(interactionSource)
        val style = Style {
            textMotion(TextMotion.Animated)
            fontSize(20.sp)
            pressed {
                animate(spec = tween(1000)) { fontSize(40.sp) }
            }
        }
        Box(
            Modifier.clickable(interactionSource = interactionSource, indication = null, onClick = {})
                .styleable(styleState = styleState, style = style)
                .testTag("hareketli")
        ) {
            BasicText("Animated Smooth Text")
        }
    }
}

fn simple_style_sample() {
    let rule = KompozTestRule::new(SimpleStyle);
    let b = bounds(&rule, "yeşil");
    assert_eq!((b.width(), b.height()), (150.0, 150.0));
    assert_eq!(rect_colors(&rule.draw()), vec![Color::Green]);
}

fn style_state_sample() {
    let enabled = mutable_state_of(true);
    let e = enabled.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || StyleStates(e.get())));
    rule.wait_for_idle();
    assert_eq!(background(&rule), Color::Green);
    let center = bounds(&rule, "durumlu").center();
    down(&rule, center);
    rule.wait_for_idle();
    assert_eq!(background(&rule), Color::Red);
    up(&rule, center);
    rule.wait_for_idle();
    assert_eq!(background(&rule), Color::Green);
    // (a mouse over it: hovered)
    rule.host().pointer_move(center, -1, 0);
    rule.wait_for_idle();
    assert_eq!(background(&rule), Color::Yellow);
    rule.host().pointer_exit();
    rule.wait_for_idle();
    enabled.set(false);
    rule.wait_for_idle();
    assert_eq!(background(&rule), Color::Gray);
}

fn style_animation_sample() {
    let rule = KompozTestRule::new(StyleAnimations);
    rule.wait_for_idle();
    assert_eq!(rect_colors(&rule.draw()), vec![Color::Blue]);
    rule.main_clock().set_auto_advance(false);
    let center = bounds(&rule, "canlı").center();
    down(&rule, center);
    rule.advance_frame();
    rule.advance_time_by(50);
    // (half way from blue to red: a linear tween of 100 ms)
    let c = background(&rule);
    assert!(c.red() > 0.2 && c.red() < 0.8 && c.blue() > 0.2 && c.blue() < 0.8, "{c:?}");
    rule.advance_time_by(100);
    assert_eq!(background(&rule), Color::Red);
    // (released: back to blue, by the default spring the property keeps animating with)
    up(&rule, center);
    rule.advance_frame();
    rule.advance_time_by(16);
    let c = background(&rule);
    assert!(c.red() > 0.0 && c.blue() < 1.0, "{c:?}");
    rule.main_clock().set_auto_advance(true);
    rule.wait_for_idle();
    assert_eq!(background(&rule), Color::Blue);
}

fn style_foreground_background_sample() {
    let rule = KompozTestRule::new(StyleForegroundBackground);
    rule.wait_for_idle();
    assert_eq!(rect_colors(&rule.draw()), vec![Color::Blue]);
    let center = bounds(&rule, "örtülü").center();
    down(&rule, center);
    rule.wait_for_idle();
    let c = rule.draw();
    // (the background, the text, then the overlay over it; the clickable's indication over all)
    assert_eq!(rect_colors(&c)[..2], [Color::Blue, named_args!(Color::Black.copy(alpha = 0.4))]);
    assert_eq!(c.texts, vec!["Hello".to_string()]);
}

fn text_motion_style_sample() {
    ComposeFoundationFlags::set_is_inherited_text_style_enabled(true);
    let rule = KompozTestRule::new(TextMotionStyle);
    rule.wait_for_idle();
    let small = bounds(&rule, "hareketli").height();
    let center = bounds(&rule, "hareketli").center();
    down(&rule, center);
    rule.wait_for_idle();
    let large = bounds(&rule, "hareketli").height();
    ComposeFoundationFlags::set_is_inherited_text_style_enabled(false);
    assert!(large > small * 1.5, "{small} → {large}");
}

// ---- material3 1.5's text field containers ---------------------------------------------------------------------

kompoz! {
    // (TextFieldDefaults.Container and OutlinedTextFieldDefaults.Container of material3 1.5.0-alpha17, their colors
    // and thicknesses given: the style's state follows the field's interaction source, focused animating)
    @Composable
    fun FilledContainer(
        enabled: Boolean,
        interactionSource: InteractionSource,
        modifier: Modifier = Modifier,
        shape: Shape = RoundedCornerShape(4.dp),
        focusedColor: Color,
        unfocusedColor: Color,
        animationSpec: FiniteAnimationSpec<Float>,
    ) {
        val styleState = remember(interactionSource) { MutableStyleState(interactionSource) }
        Box(
            modifier
                .styleable(styleState) {
                    shape(shape)
                    background(if (enabled) unfocusedColor else Color.Gray)
                    focused {
                        animate(animationSpec) {
                            background(focusedColor)
                        }
                    }
                }
        )
    }

    @Composable
    fun OutlinedContainer(
        interactionSource: InteractionSource,
        modifier: Modifier = Modifier,
        shape: Shape = RoundedCornerShape(4.dp),
        focusedBorderThickness: Dp = 2.dp,
        unfocusedBorderThickness: Dp = 1.dp,
        animationSpec: FiniteAnimationSpec<Float>,
    ) {
        val styleState = remember(interactionSource) { MutableStyleState(interactionSource) }
        Box(
            modifier.styleable(styleState) {
                shape(shape)
                background(Color.White)
                border(unfocusedBorderThickness, Color.Gray)
                focused {
                    animate(animationSpec) {
                        background(Color.White)
                        border(focusedBorderThickness, Color.Blue)
                    }
                }
            }
        )
    }

    @Composable
    fun Fields() {
        val filled = remember { MutableInteractionSource() }
        val outlined = remember { MutableInteractionSource() }
        Column {
            FilledContainer(
                enabled = true,
                interactionSource = filled,
                modifier = Modifier.size(100.dp, 40.dp).focusable(interactionSource = filled).testTag("dolu"),
                focusedColor = Color.Cyan,
                unfocusedColor = Color.LightGray,
                animationSpec = tween(100, easing = LinearEasing),
            )
            OutlinedContainer(
                interactionSource = outlined,
                modifier = Modifier.size(100.dp, 40.dp).focusable(interactionSource = outlined).testTag("çizgili"),
                animationSpec = snap(),
            )
        }
    }
}

fn material3_text_field_containers() {
    let rule = KompozTestRule::new(Fields);
    rule.wait_for_idle();
    let c = rule.draw();
    // (the filled one's background in its shape, the outlined one's border 1 px)
    assert_eq!(c.rect_colors[0], Color::LightGray);
    assert_eq!(c.stroke_colors, vec![Color::Gray]);
    rule.on_node_with_tag("dolu").request_focus();
    rule.wait_for_idle();
    assert_eq!(rule.draw().rect_colors[0], Color::Cyan);
    rule.on_node_with_tag("çizgili").request_focus();
    rule.wait_for_idle();
    let c = rule.draw();
    assert_eq!(c.rect_colors[0], Color::LightGray);
    assert_eq!(c.stroke_colors, vec![Color::Blue]);
}

// ---- Custom states, combinations, locals and the state read ---------------------------------------------------

kompoz! {
    // (StyleStateKeySample's key and its block, `state(key) { }`; the state set by the component, Kotlin's
    // `state[key] = value`)
    val PlayingKey = StyleStateKey(false)

    @Composable
    fun MediaPlayer(playing: Boolean) {
        val styleState = rememberUpdatedStyleState(null) { state -> state[PlayingKey] = playing }
        Box(Modifier.styleable(styleState) {
            size(20.dp)
            borderWidth(2.dp)
            borderColor(Color.Gray)
            state(PlayingKey) { borderColor(Color.Green) }
        })
    }

    val BaseStyle = Style {
        size(20.dp)
        background(Color.Red)
    }

    @Composable
    fun Combined(blue: Boolean) {
        val over = Style { background(Color.Blue) }
        Column {
            Box(Modifier.styleable(null, if (blue) Style(BaseStyle, over) else BaseStyle then Style))
            Box(Modifier.styleable(null, Style(BaseStyle, Style { alpha(0.5f) }, over)))
        }
    }

    val LocalTint = compositionLocalOf { Color.Black }

    @Composable
    fun LocalsAndState(tint: Color) {
        val interactionSource = remember { MutableInteractionSource() }
        CompositionLocalProvider(LocalTint provides tint) {
            Box(
                Modifier
                    .clickable(interactionSource = interactionSource, indication = null, onClick = {})
                    .styleable(rememberUpdatedStyleState(interactionSource)) {
                        size(30.dp)
                        background(if (state.isPressed) Color.Red else LocalTint.currentValue)
                        contentPadding(PaddingValues(start = 4.dp))
                    }
                    .testTag("yerel")
            ) {
                Box(Modifier.fillMaxSize().testTag("yerel iç"))
            }
        }
    }
}

fn custom_state_keys_as_compose_writes_them() {
    let playing = mutable_state_of(false);
    let p = playing.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || MediaPlayer(p.get())));
    assert_eq!(rule.draw().stroke_colors, vec![Color::Gray]);
    playing.set(true);
    rule.wait_for_idle();
    assert_eq!(rule.draw().stroke_colors, vec![Color::Green]);
}

fn styles_combined_as_compose_writes_them() {
    let blue = mutable_state_of(false);
    let b = blue.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || Combined(b.get())));
    let c = rule.draw();
    assert_eq!(c.rect_colors[0], Color::Red);
    // (the last one wins: blue, at half alpha)
    assert_eq!(c.rect_colors[1], Color::Blue);
    assert!(c.layer_alphas.iter().any(|&a| (a - 0.5).abs() < 0.01));
    blue.set(true);
    rule.wait_for_idle();
    assert_eq!(rule.draw().rect_colors[0], Color::Blue);
}

fn locals_and_the_state_read_in_a_style() {
    let tint = mutable_state_of(Color::Green);
    let t = tint.clone();
    let rule = KompozTestRule::new(kompozable_lambda!(move || LocalsAndState(t.get())));
    assert_eq!(rule.draw().rect_colors[0], Color::Green);
    // (the padding of a PaddingValues, its start in the layout's direction)
    assert_eq!(rule.on_node_with_tag_unmerged("yerel iç").get_unclipped_bounds_in_root().left, 4.0);
    tint.set(Color::Yellow);
    rule.wait_for_idle();
    assert_eq!(rule.draw().rect_colors[0], Color::Yellow);
    let center = bounds(&rule, "yerel").center();
    down(&rule, center);
    rule.wait_for_idle();
    assert_eq!(rule.draw().rect_colors[0], Color::Red);
    up(&rule, center);
}

// ---- kompoz_file! ------------------------------------------------------------------------------------------------

kompoz_file!("tests/kt/style.kt");

fn a_kt_files_styles() {
    let rule = KompozTestRule::new(StyleFile);
    rule.wait_for_idle();
    let c = rule.draw();
    assert_eq!(c.rect_colors[0], Color::Magenta);
    let card = bounds(&rule, "kart");
    assert_eq!((card.width(), card.height()), (64.0, 32.0));
    // (its layer moved 4 dp right, the content 8 dp in by the padding)
    let inside = bounds(&rule, "kart iç");
    assert_eq!((inside.left, inside.top, inside.width()), (12.0, 4.0, 48.0));
    // (a drop shadow under it)
    assert_eq!(c.shadows.len(), 1);
    let center = card.center();
    rule.host().pointer_move(center, -1, 0);
    rule.wait_for_idle();
    let c = rule.draw();
    assert_eq!(c.rect_colors[0], Color::Cyan);
    assert_eq!(c.stroke_colors, vec![Color::Blue]);
}

tests![
    material3_text_field_containers,
    custom_state_keys_as_compose_writes_them,
    styles_combined_as_compose_writes_them,
    locals_and_the_state_read_in_a_style,
    a_kt_files_styles,
    simple_style_sample,
    style_state_sample,
    style_animation_sample,
    style_foreground_background_sample,
    text_motion_style_sample,
];
