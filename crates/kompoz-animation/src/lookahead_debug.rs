//! Lookahead animations' visual debugging, as Compose's `LookaheadAnimationVisualDebugHelper.kt`,
//! `LookaheadAnimationVisualDebugConfig.kt`, `IsLookaheadAnimationVisualDebuggingEnabled.kt` and `CompositionLocals.kt`
//! (Compose's `@ExperimentalLookaheadAnimationVisualDebugApi`): [`LookaheadAnimationVisualDebugging`] turns it on for
//! its content — `animate_bounds` (and shared elements) then draw where they are going, the path they take there and
//! where they are on it — and [`CustomizedLookaheadAnimationVisualDebugging`] gives its content's animations a color
//! of their own.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the shared transition parts are SharedTransitionScope's (stage 11 WP4): tested here until then"
    )
)]

use crate::converter::VectorConverter;
use crate::{
    Animatable, Animation, ArcAnimationSpec, ArcMode, FiniteAnimationSpec, LinearEasing, RepeatMode, SnapSpec,
    TargetBasedAnimation, TweenSpec, infinite_repeatable, tween,
};
use kompoz_coroutines::CoroutineScope;
use kompoz_graphics::{
    Brush, Color, Dp, DrawStyle, IntSize, Offset, Path, PathEffect, Rect, Size, Stroke, StrokeCap, StrokeJoin,
    TileMode, UnitExt,
};
use kompoz_runtime::{KompozitionLocal, KompozitionLocalProvider, kompozable, named_args};
use kompoz_text::{FontWeight, TextMeasurer, TextStyle};
use kompoz_ui::draw::{DrawScope, Paint, PaintingStyle};
use kompoz_ui::graphics_context::ClipOp;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;

/// Whether the visual debugging is built in (Compose's `isLookaheadAnimationVisualDebuggingEnabled`, which R8 turns
/// off in release builds): it is; it draws only inside a [`LookaheadAnimationVisualDebugging`] that enables it.
pub(crate) const IS_LOOKAHEAD_ANIMATION_VISUAL_DEBUGGING_ENABLED: bool = true;

/// What a [`LookaheadAnimationVisualDebugging`] sets for its content (Compose's internal
/// LookaheadAnimationVisualDebugConfig): whether it is on, the film over what lies under the overlay, the colors of
/// shared element keys with several matches, without one, of inactive elements, and whether keys are written.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct LookaheadAnimationVisualDebugConfig {
    pub(crate) is_enabled: bool,
    pub(crate) overlay_color: Color,
    pub(crate) multiple_matches_color: Color,
    pub(crate) unmatched_element_color: Color,
    pub(crate) inactive_element_color: Color,
    pub(crate) is_show_key_label_enabled: bool,
}

impl Default for LookaheadAnimationVisualDebugConfig {
    /// Compose's defaults (on).
    fn default() -> Self {
        LookaheadAnimationVisualDebugConfig {
            is_enabled: true,
            overlay_color: Color::from_argb(0x8034A853),
            multiple_matches_color: Color::from_argb(0xFFEA4335),
            unmatched_element_color: Color::from_argb(0xFF9AA0A6),
            inactive_element_color: Color::from_argb(0xFF000000),
            is_show_key_label_enabled: false,
        }
    }
}

/// The debugging configuration where one is read (Compose's internal LocalLookaheadAnimationVisualDebugConfig):
/// off unless a [`LookaheadAnimationVisualDebugging`] around turns it on.
pub(crate) static LocalLookaheadAnimationVisualDebugConfig: KompozitionLocal<LookaheadAnimationVisualDebugConfig> =
    KompozitionLocal::new_static(|| LookaheadAnimationVisualDebugConfig { is_enabled: false, ..Default::default() });

/// The color of the animations where it is read (Compose's internal LocalLookaheadAnimationVisualDebugColor):
/// unspecified (each animation gets one of the helper's colors) unless a
/// [`CustomizedLookaheadAnimationVisualDebugging`] around gives one.
pub(crate) static LocalLookaheadAnimationVisualDebugColor: KompozitionLocal<Color> =
    KompozitionLocal::new_static(|| Color::Unspecified);

/// Turns the visual debugging of shared element and `animate_bounds` animations on (or off) for everything
/// `content` composes, however deep (Compose's `LookaheadAnimationVisualDebugging`): an animating element is outlined,
/// with its target outlined and the path between, dashed, and a diamond where it is along it; an inactive one outlined
/// in `inactive_element_color`; in a shared transition, `overlay_color` films what lies under the overlay, and keys
/// with several matches (`multiple_matches_color`) or none (`unmatched_element_color`) are marked; keys are written
/// with `is_show_key_label_enabled`. What shows depends on where the modifiers are.
///
/// Kotlin: `LookaheadAnimationVisualDebugging(isEnabled: Boolean = true, overlayColor: Color = Color(0x8034A853), multipleMatchesColor: Color = Color(0xFFEA4335), unmatchedElementColor: Color = Color(0xFF9AA0A6), inactiveElementColor: Color = Color(0xFF000000), isShowKeyLabelEnabled: Boolean = false, content: @Composable () -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn LookaheadAnimationVisualDebugging(
    #[default(true)] is_enabled: bool,
    #[default(Color::from_argb(0x8034A853))] overlay_color: Color,
    #[default(Color::from_argb(0xFFEA4335))] multiple_matches_color: Color,
    #[default(Color::from_argb(0xFF9AA0A6))] unmatched_element_color: Color,
    #[default(Color::from_argb(0xFF000000))] inactive_element_color: Color,
    #[default(false)] is_show_key_label_enabled: bool,
    content: impl Fn() + 'static,
) {
    let config = LookaheadAnimationVisualDebugConfig {
        is_enabled,
        overlay_color,
        multiple_matches_color,
        unmatched_element_color,
        inactive_element_color,
        is_show_key_label_enabled,
    };
    KompozitionLocalProvider(&[LocalLookaheadAnimationVisualDebugConfig.provides(config)], content);
}

/// Gives the shared element and `animate_bounds` animations of everything `content` composes, however deep,
/// `debug_color` in their visual debugging (Compose's `CustomizedLookaheadAnimationVisualDebugging`).
///
/// Kotlin: `CustomizedLookaheadAnimationVisualDebugging(debugColor: Color, content: @Composable () -> Unit)`
#[kompozable]
pub fn CustomizedLookaheadAnimationVisualDebugging(debug_color: Color, content: impl Fn() + 'static) {
    KompozitionLocalProvider(&[LocalLookaheadAnimationVisualDebugColor.provides(debug_color)], content);
}

thread_local! {
    /// The next of the helper's colors for an animation without one of its own (Compose's `colorIndex`).
    static COLOR_INDEX: Cell<usize> = const { Cell::new(0) };
    /// The color each key was given, so a key keeps its color (Compose's `keyToColor`).
    static KEY_TO_COLOR: RefCell<HashMap<String, Color>> = RefCell::new(HashMap::new());
}

/// The colors animations without one of their own are given in turn (Compose's helper's `colors`).
const COLORS: [Color; 5] = [
    Color::from_argb(0xFFEA4335),
    Color::from_argb(0xFFF29027),
    Color::from_argb(0xFFD148DB),
    Color::from_argb(0xFF4285F4),
    Color::from_argb(0xFF3AB8BA),
];

/// White at an alpha (Compose's `Color.White.copy(alpha = …)`).
fn white(alpha: f32) -> Color {
    named_args!(Color::White.copy(alpha = alpha))
}

/// A stroke `width` wide, optionally dashed.
fn stroke(width: f32, path_effect: Option<PathEffect>) -> DrawStyle {
    DrawStyle::Stroke(Stroke::new(width, Stroke::DefaultMiter, StrokeCap::Butt, StrokeJoin::Miter, path_effect))
}

/// The dashes the paths are drawn with (Compose's `dashPathEffect(floatArrayOf(20f, 10f))`).
fn dashes() -> PathEffect {
    PathEffect::dash_path_effect(&[20.0, 10.0], 0.0)
}

/// What draws a lookahead animation's visual debugging (Compose's LookaheadAnimationVisualDebugHelper): the outlines,
/// the path of an animation and where it is on it, the overlay and the perimeter of a shared transition, marks of
/// keys matched more than once or not at all.
pub(crate) struct LookaheadAnimationVisualDebugHelper {
    reverse_progress: Animatable<f32, crate::AnimationVector1D>,
    restart_progress: Animatable<f32, crate::AnimationVector1D>,
    is_progress_animation_running: Cell<bool>,
    shared_transition_scope_offset: Cell<Offset>,
    shared_transition_scope_size: Cell<IntSize>,
    /// Where the path's end is from its start (the target's center from the start's).
    debug_offset: Cell<Offset>,
    /// The path the animation takes, its start at the origin.
    debug_path: RefCell<Path>,
    /// The diamond where it is along the path.
    center_path: RefCell<Path>,
}

impl LookaheadAnimationVisualDebugHelper {
    pub(crate) fn new() -> LookaheadAnimationVisualDebugHelper {
        LookaheadAnimationVisualDebugHelper {
            reverse_progress: Animatable(0.0f32),
            restart_progress: Animatable(0.0f32),
            is_progress_animation_running: Cell::new(false),
            shared_transition_scope_offset: Cell::new(Offset::Zero),
            shared_transition_scope_size: Cell::new(IntSize::Zero),
            debug_offset: Cell::new(Offset::Zero),
            debug_path: RefCell::new(Path::new()),
            center_path: RefCell::new(Path::new()),
        }
    }

    /// Starts the progress animations of a shared transition's perimeter (Compose's `onAttach`): a back and forth
    /// over 500 ms (its width) and a loop of 1 s (its gradient's way round).
    pub(crate) fn on_attach(&self, coroutine_scope: &CoroutineScope) {
        if self.is_progress_animation_running.get() {
            return;
        }
        self.is_progress_animation_running.set(true);
        let reverse = self.reverse_progress.clone();
        coroutine_scope.launch(async move {
            reverse.snap_to(0.0).await;
            let spec = named_args!(infinite_repeatable(
                named_args!(tween(500, easing = Arc::new(LinearEasing))),
                repeat_mode = RepeatMode::Reverse
            ));
            named_args!(reverse.animate_to(1.0, animation_spec = Arc::new(spec))).await;
        });
        let restart = self.restart_progress.clone();
        coroutine_scope.launch(async move {
            restart.snap_to(0.0).await;
            let spec = named_args!(infinite_repeatable(
                named_args!(tween(1000, easing = Arc::new(LinearEasing))),
                repeat_mode = RepeatMode::Restart
            ));
            named_args!(restart.animate_to(1.0, animation_spec = Arc::new(spec))).await;
        });
    }

    /// Stops them (Compose's `onDetach`).
    pub(crate) fn on_detach(&self, coroutine_scope: &CoroutineScope) {
        if !self.is_progress_animation_running.get() {
            return;
        }
        self.is_progress_animation_running.set(false);
        let (reverse, restart) = (self.reverse_progress.clone(), self.restart_progress.clone());
        coroutine_scope.launch(async move {
            reverse.stop().await;
            restart.stop().await;
        });
    }

    /// Where the shared transition's scope is and its size (Compose's `updateDrawingCoordinates`).
    pub(crate) fn update_drawing_coordinates(&self, offset_in_shared_transition_scope: Offset, size: IntSize) {
        self.shared_transition_scope_offset.set(offset_in_shared_transition_scope);
        self.shared_transition_scope_size.set(size);
    }

    /// The shared transition's perimeter, stroked with a gradient going round it, its width breathing (Compose's
    /// `drawGlobalVisualizations`).
    pub(crate) fn draw_global_visualizations(&self, scope: &mut DrawScope) {
        let size = self.shared_transition_scope_size.get();
        let animated_distance = (2 * size.width + 2 * size.height) as f32 * self.restart_progress.value();
        let perimeter_center = self.find_position_along_perimeter(animated_distance);
        let gradient_colors = [
            Color::from_argb(0xFFEA4335),
            Color::from_argb(0xFF4285F4),
            Color::from_argb(0xFF34A853),
            Color::from_argb(0xFFFBBC04),
            Color::from_argb(0xFFEA4335),
        ];
        let perimeter_brush = Brush::radial_gradient(&gradient_colors, perimeter_center, 2000.0, TileMode::Clamp);
        let stroke_width =
            scope.density().to_px(Dp(8.0)) * self.reverse_progress.value() + scope.density().to_px(Dp(4.0));
        let draw_size = scope.size();
        let offset = self.shared_transition_scope_offset.get();
        scope.draw_into_canvas(|canvas| {
            // (Compose's `Brush.applyTo(size, paint, 1f)`: the gradient's shader over the draw size)
            let paint = Paint {
                shader: Some(perimeter_brush.create_shader(draw_size)),
                style: PaintingStyle::Stroke,
                stroke_width,
                ..Paint::default()
            };
            canvas.save();
            canvas.translate(-offset.x, -offset.y);
            canvas.draw_rect(Rect::new(0.0, 0.0, size.width as f32, size.height as f32), &paint);
            canvas.restore();
        });
    }

    /// The film over what lies under the overlay (Compose's `drawOverlay`).
    pub(crate) fn draw_overlay(&self, scope: &mut DrawScope, overlay_color: Color) {
        scope.draw_rect(overlay_color);
    }

    /// The color of `key`'s animation: the one it was given, else the next of the helper's (Compose's
    /// `chooseColor`).
    pub(crate) fn choose_color(&self, key: &str) -> Color {
        KEY_TO_COLOR.with(|map| {
            let mut map = map.borrow_mut();
            if let Some(c) = map.get(key) {
                return *c;
            }
            let index = COLOR_INDEX.with(|i| {
                if i.get() >= COLORS.len() {
                    i.set(0);
                }
                let at = i.get();
                i.set(at + 1);
                at
            });
            let color = COLORS[index];
            map.insert(key.to_string(), color);
            color
        })
    }

    /// An inactive element: outlined (Compose's `drawInactiveVisualizations`), its key written with
    /// `is_show_key_label_enabled`. Nothing in transparent (for tests).
    pub(crate) fn draw_inactive_visualizations(
        &self,
        scope: &mut DrawScope,
        inactive_element_color: Color,
        is_show_key_label_enabled: bool,
        stroke_width: f32,
        key: &str,
        text_measurer: Option<&TextMeasurer>,
    ) {
        if inactive_element_color == Color::Transparent {
            return;
        }
        let highlight_width = stroke_width * 2.0;
        named_args!(scope.draw_rect(Color::White, style = stroke(highlight_width, None)));
        named_args!(scope.draw_rect(inactive_element_color, style = stroke(stroke_width, None)));
        if is_show_key_label_enabled && let Some(measurer) = text_measurer {
            let style = label_style(inactive_element_color, 18.0, white(0.6), None);
            let layout = named_args!(measurer.measure(key, style = style));
            named_args!(scope.draw_text(&layout, top_left = Offset::new(10.0, 10.0)));
        }
    }

    /// An animating element (Compose's `drawLocalVisualizations`): outlined where it is, its target outlined, the path
    /// between dashed and a diamond at `center` — in `animation_color`, or (unspecified) in its key's color over a
    /// white highlight. Its key written with `is_show_key_label_enabled`, here and at the target. Nothing in
    /// transparent (for tests).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_local_visualizations(
        &self,
        scope: &mut DrawScope,
        animation_color: Color,
        target_offset: Offset,
        target_size: Size,
        current_rect: Rect,
        center: Offset,
        is_show_key_label_enabled: bool,
        stroke_width: f32,
        key: &str,
        text_measurer: Option<&TextMeasurer>,
    ) {
        if animation_color == Color::Transparent {
            return;
        }
        let highlight_width = stroke_width * 2.0;
        let to_target = target_offset - current_rect.top_left();
        let path_at = to_target - self.debug_offset.get();
        let chosen_color = if animation_color != Color::Unspecified {
            animation_color
        } else {
            // The white highlights under the key's color: the outline, the target's, the path, the diamond.
            named_args!(scope.draw_rect(Color::White, style = stroke(highlight_width, None)));
            scope.translate_with(to_target.x, to_target.y, |s| {
                named_args!(s.draw_rect(Color::White, size = target_size, style = stroke(highlight_width, None)));
            });
            let path = self.debug_path.borrow().clone();
            scope.translate_with(path_at.x, path_at.y, |s| {
                s.translate_with(target_size.width * 0.5, target_size.height * 0.5, |s| {
                    named_args!(s.draw_path(&path, Color::White, style = stroke(highlight_width, Some(dashes()))));
                });
            });
            self.calculate_path_center(stroke_width * 3.5);
            let diamond = self.center_path.borrow().clone();
            scope.translate_with(center.x, center.y, |s| s.draw_path(&diamond, Color::White));
            self.choose_color(key)
        };
        named_args!(scope.draw_rect(chosen_color, style = stroke(stroke_width, None)));
        scope.translate_with(to_target.x, to_target.y, |s| {
            named_args!(s.draw_rect(chosen_color, size = target_size, style = stroke(stroke_width, None)));
        });
        let path = self.debug_path.borrow().clone();
        scope.translate_with(path_at.x, path_at.y, |s| {
            s.translate_with(target_size.width * 0.5, target_size.height * 0.5, |s| {
                named_args!(s.draw_path(&path, chosen_color, style = stroke(stroke_width, Some(dashes()))));
            });
        });
        self.calculate_path_center(stroke_width * 3.0);
        let diamond = self.center_path.borrow().clone();
        scope.translate_with(center.x, center.y, |s| s.draw_path(&diamond, chosen_color));
        if is_show_key_label_enabled && let Some(measurer) = text_measurer {
            let style = label_style(chosen_color, 18.0, white(0.6), None);
            let layout = named_args!(measurer.measure(key, style = style));
            named_args!(scope.draw_text(&layout, top_left = Offset::new(10.0, 10.0)));
            scope.translate_with(to_target.x, to_target.y, |s| {
                named_args!(s.draw_text(&layout, top_left = Offset::new(10.0, 10.0)));
            });
        }
    }

    /// A shared element key with `num_matches` matches (Compose's `drawMultipleMatchesElement`): outlined, its key
    /// and the count written with `is_show_key_label_enabled`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn draw_multiple_matches_element(
        &self,
        scope: &mut DrawScope,
        multiple_matches_color: Color,
        is_show_key_label_enabled: bool,
        key: &str,
        num_matches: usize,
        text_measurer: &TextMeasurer,
        stroke_width: f32,
    ) {
        let highlight_width = stroke_width * 2.0;
        named_args!(scope.draw_rect(Color::White, style = stroke(highlight_width, None)));
        named_args!(scope.draw_rect(multiple_matches_color, style = stroke(stroke_width, None)));
        if is_show_key_label_enabled {
            let emoji = match num_matches {
                2..=9 => format!("{num_matches}\u{FE0F}\u{20E3}"),
                _ => "> 9\u{FE0F}\u{20E3}".to_string(),
            };
            let style = label_style(
                Color::White,
                22.0,
                named_args!(multiple_matches_color.copy(alpha = 0.8)),
                Some(FontWeight::Bold),
            );
            let layout = measurer_measure(text_measurer, format!("{key}: {emoji} matches"), style);
            named_args!(scope.draw_text(&layout, top_left = Offset::new(10.0, 10.0)));
        }
    }

    /// A shared element key without a match (Compose's `drawUnmatchedElement`): outlined and hatched, its key written
    /// with `is_show_key_label_enabled`.
    pub(crate) fn draw_unmatched_element(
        &self,
        scope: &mut DrawScope,
        unmatched_color: Color,
        is_show_key_label_enabled: bool,
        key: &str,
        text_measurer: &TextMeasurer,
        stroke_width: f32,
    ) {
        let highlight_width = stroke_width * 2.0;
        named_args!(scope.draw_rect(Color::White, style = stroke(highlight_width, None)));
        named_args!(scope.draw_rect(unmatched_color, style = stroke(stroke_width, None)));
        let size = scope.size();
        scope.clip_rect_with(0.0, 0.0, size.width, size.height, ClipOp::Intersect, |s| {
            let (w, h) = (size.width, size.height);
            // (from before the left edge)
            let mut x = -h;
            while x < w {
                named_args!(s.draw_line(
                    named_args!(unmatched_color.copy(alpha = 0.3)),
                    Offset::new(x, h),
                    Offset::new(x + h, 0.0),
                    stroke_width = stroke_width
                ));
                x += stroke_width * 4.0;
            }
        });
        if is_show_key_label_enabled {
            let style =
                label_style(Color::White, 22.0, named_args!(unmatched_color.copy(alpha = 0.8)), Some(FontWeight::Bold));
            let layout = measurer_measure(text_measurer, format!("{key}: 0\u{FE0F}\u{20E3} matches"), style);
            named_args!(scope.draw_text(&layout, top_left = Offset::new(10.0, 10.0)));
        }
    }

    /// The point `distance_traveled` along the scope's perimeter, clockwise from its top left (Compose's
    /// `findPositionAlongPerimeter`).
    fn find_position_along_perimeter(&self, distance_traveled: f32) -> Offset {
        let size = self.shared_transition_scope_size.get();
        let (width, height) = (size.width as f32, size.height as f32);
        if distance_traveled <= width {
            return Offset::new(distance_traveled, 0.0);
        }
        if distance_traveled <= width + height {
            return Offset::new(width, distance_traveled - width);
        }
        if distance_traveled <= width * 2.0 + height {
            return Offset::new(width * 2.0 + height - distance_traveled, height);
        }
        Offset::new(0.0, height * 2.0 + width * 2.0 - distance_traveled)
    }

    /// The diamond, `diamond_width` from its center to each corner (Compose's `calculatePathCenter`).
    fn calculate_path_center(&self, diamond_width: f32) {
        let mut path = self.center_path.borrow_mut();
        path.rewind();
        path.move_to(0.0, -diamond_width);
        path.line_to(diamond_width, 0.0);
        path.line_to(0.0, diamond_width);
        path.line_to(-diamond_width, 0.0);
        path.close();
    }

    /// The path of an animation along `spec` from `current` to `target` (its centers'), from `initial_velocity`, its
    /// start at the origin (Compose's `calculatePath`): a line for a tween, a snap or a linear arc; otherwise the
    /// animation's course sampled at 400 points.
    pub(crate) fn calculate_path(
        &self,
        spec: &Arc<dyn FiniteAnimationSpec<Rect>>,
        current: Rect,
        target: Rect,
        initial_velocity: Rect,
    ) {
        let mut path = self.debug_path.borrow_mut();
        path.rewind();
        let any = spec.as_any();
        let linear = any.is::<TweenSpec<Rect>>()
            || any.is::<SnapSpec<Rect>>()
            || any.downcast_ref::<ArcAnimationSpec<Rect>>().is_some_and(|a| a.mode == ArcMode::ArcLinear);
        if linear {
            path.move_to(current.center().x, current.center().y);
            path.line_to(target.center().x, target.center().y);
            path.translate(-current.center());
            self.debug_offset.set(target.center() - current.center());
            return;
        }
        let path_anim =
            TargetBasedAnimation::with_velocity(&**spec, Rect::VectorConverter, current, target, initial_velocity);
        let duration_nanos = path_anim.duration_nanos();
        let start_value = path_anim.get_value_from_nanos(0);
        let num_steps = 400;
        for i in 0..=num_steps {
            let play_time =
                duration_nanos - (duration_nanos as f64 * (i as f32 / (num_steps - 1) as f32) as f64) as i64;
            let point = path_anim.get_value_from_nanos(play_time).center();
            if i == 0 {
                path.move_to(point.x, point.y);
            } else {
                path.line_to(point.x, point.y);
            }
        }
        path.translate(-start_value.center());
        self.debug_offset.set(target.center() - start_value.center());
    }
}

/// The style of a key's label: `font_size` sp in `color` on `background`, in `font_weight`.
fn label_style(color: Color, font_size: f32, background: Color, font_weight: Option<FontWeight>) -> TextStyle {
    TextStyle { color, font_size: font_size.sp(), background, font_weight, ..TextStyle::Default }
}

/// `text` laid out in `style` by `text_measurer`.
fn measurer_measure(text_measurer: &TextMeasurer, text: String, style: TextStyle) -> kompoz_text::TextLayoutResult {
    named_args!(text_measurer.measure(text, style = style))
}
