//! Optical centering, as Compose's HorizontalCenterOptically.kt: content in a shape whose corners differ side to
//! side (a split button's halves, a connected button group's ends) looks off center when it is centered; it is moved
//! toward the rounder side by a share of the difference ([`CenterOpticallyCoefficient`]), within the offsets
//! allowed.

// (the split buttons' and button groups', used as they come)
#![allow(dead_code)]

use crate::internal::animated_shape::AnimatedShape;
use kompoz_graphics::{CornerBasedShape, Dp, RoundedCornerShape, Shape, Size};
use kompoz_runtime::named;
use kompoz_ui::Modifier;
use std::any::Any;
use std::sync::Arc;

/// What a shape's corners at the start and at the end differ by that the content moves by (Compose's internal
/// `CenterOpticallyCoefficient`).
pub(crate) const CenterOpticallyCoefficient: f32 = 0.11;

/// A shape that says how far its content moves to look centered (Compose's internal interface
/// `ShapeWithHorizontalCenterOptically`): the animated shapes'.
pub(crate) trait ShapeWithHorizontalCenterOptically: Shape {
    /// How far to the end its content moves, px (to the start where negative).
    fn offset(&self) -> f32;
}

/// `shape` as a [`ShapeWithHorizontalCenterOptically`] if it is one (Kotlin's `is
/// ShapeWithHorizontalCenterOptically`): the animated shapes are.
pub(crate) fn as_shape_with_horizontal_center_optically(
    shape: &Arc<dyn Shape>,
) -> Option<Arc<dyn ShapeWithHorizontalCenterOptically>> {
    let any: Arc<dyn Any + Send + Sync> = shape.clone();
    let any = match any.downcast::<AnimatedShape<RoundedCornerShape>>() {
        Ok(s) => return Some(s),
        Err(any) => any,
    };
    match any.downcast::<AnimatedShape<CornerBasedShape>>() {
        Ok(s) => Some(s),
        Err(_) => None,
    }
}

/// Kotlin's `Float.coerceIn(min, max)`: an empty range throws.
fn coerce_in(value: f32, minimum_value: f32, maximum_value: f32) -> f32 {
    if minimum_value > maximum_value {
        kompoz_runtime::kt::panic_kt(
            "IllegalArgumentException",
            &format!(
                "Cannot coerce value to an empty range: maximum {maximum_value} is less than minimum {minimum_value}."
            ),
        );
    }
    if value < minimum_value {
        minimum_value
    } else if value > maximum_value {
        maximum_value
    } else {
        value
    }
}

/// Kotlin's `Float.roundToInt` (Math.round: a half up).
fn round_to_int(value: f32) -> i32 {
    (value + 0.5).floor() as i32
}

/// The optical centering modifiers (Compose's internal `Modifier.horizontalCenterOptically` overloads).
#[named(pub(crate) trait HorizontalCenterOpticallyModifiers)]
impl Modifier {
    /// What it wraps moved toward the rounder side of `shape`: by [`CenterOpticallyCoefficient`] of the difference
    /// between the average of its start corners and of its end corners, in the size it is measured to, at most
    /// `max_start_offset` to the start and `max_end_offset` to the end (Compose's internal
    /// `Modifier.horizontalCenterOptically(shape: CornerBasedShape, maxStartOffset = 0.dp, maxEndOffset = 0.dp)`).
    fn horizontal_center_optically(
        self,
        shape: CornerBasedShape,
        #[default(Dp(0.0))] max_start_offset: Dp,
        #[default(Dp(0.0))] max_end_offset: Dp,
    ) -> Modifier {
        self.horizontal_center_optically_with(shape, max_start_offset, max_end_offset)
    }

    /// [`horizontal_center_optically`](HorizontalCenterOpticallyModifiers::horizontal_center_optically) with all
    /// its parameters, for plain code.
    fn horizontal_center_optically_with(
        self,
        shape: CornerBasedShape,
        max_start_offset: Dp,
        max_end_offset: Dp,
    ) -> Modifier {
        self.layout(move |scope, measurable, constraints| {
            let placeable = measurable.measure(constraints);
            let width = placeable.width();
            let height = placeable.height();
            let size = Size::new(width as f32, height as f32);
            let density = scope.density();
            let max_start_offset_px = -density.to_px(max_start_offset);
            let max_end_offset_px = density.to_px(max_end_offset);

            let top_start = shape.top_start().to_px(size, &density);
            let top_end = shape.top_end().to_px(size, &density);
            let bottom_start = shape.bottom_start().to_px(size, &density);
            let bottom_end = shape.bottom_end().to_px(size, &density);
            let avg_start = (top_start + bottom_start) / 2.0;
            let avg_end = (top_end + bottom_end) / 2.0;
            let padding_correction = CenterOpticallyCoefficient * (avg_start - avg_end);
            scope.layout(width, height, move |p| {
                let coerced_correction = coerce_in(padding_correction, max_start_offset_px, max_end_offset_px);
                p.place_relative(&placeable, round_to_int(coerced_correction), 0);
            })
        })
    }

    /// What it wraps moved by `shape`'s [`offset`](ShapeWithHorizontalCenterOptically::offset) (read as it is
    /// placed), at most `max_start_offset` to the start and `max_end_offset` to the end (Compose's internal
    /// `Modifier.horizontalCenterOptically(shape: ShapeWithHorizontalCenterOptically, maxStartOffset = 0.dp,
    /// maxEndOffset = 0.dp)`).
    ///
    /// Kotlin: `horizontalCenterOptically(shape: ShapeWithHorizontalCenterOptically, maxStartOffset: Dp = 0.dp, maxEndOffset: Dp = 0.dp)`
    fn horizontal_center_optically_of(
        self,
        shape: Arc<dyn ShapeWithHorizontalCenterOptically>,
        #[default(Dp(0.0))] max_start_offset: Dp,
        #[default(Dp(0.0))] max_end_offset: Dp,
    ) -> Modifier {
        self.horizontal_center_optically_of_with(shape, max_start_offset, max_end_offset)
    }

    /// [`horizontal_center_optically_of`](HorizontalCenterOpticallyModifiers::horizontal_center_optically_of) with
    /// all its parameters, for plain code.
    fn horizontal_center_optically_of_with(
        self,
        shape: Arc<dyn ShapeWithHorizontalCenterOptically>,
        max_start_offset: Dp,
        max_end_offset: Dp,
    ) -> Modifier {
        self.layout(move |scope, measurable, constraints| {
            let placeable = measurable.measure(constraints);
            let width = placeable.width();
            let height = placeable.height();
            let max_start_offset_px = -scope.density().to_px(max_start_offset);
            let max_end_offset_px = scope.density().to_px(max_end_offset);
            let shape = shape.clone();
            scope.layout(width, height, move |p| {
                let coerced_offset = coerce_in(shape.offset(), max_start_offset_px, max_end_offset_px);
                p.place_relative(&placeable, round_to_int(coerced_offset), 0);
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::internal::animated_shape::remember_animated_shape;
    use kompoz_animation::{FiniteAnimationSpec, LinearEasing, TweenSpec};
    use kompoz_foundation::prelude::*;
    use kompoz_graphics::{Color, CornerSize, IntSize, LayoutDirection, Rect, UnitExt};
    use kompoz_runtime::{KompozitionLocalProvider, kompozable_lambda, mutable_state_of, named_args, remember};
    use kompoz_test::KompozTestRule;
    use kompoz_ui::platform::LocalLayoutDirection;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn bounds(rule: &KompozTestRule, tag: &str) -> Rect {
        rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
    }

    /// Rounded at the start (20 px), square at the end.
    fn rounder_at_the_start() -> CornerBasedShape {
        let (round, square) = (CornerSize::px(20.0), CornerSize::px(0.0));
        RoundedCornerShape::new(round.clone(), square.clone(), square, round).into()
    }

    /// Rounded at the end (20 px), square at the start.
    fn rounder_at_the_end() -> CornerBasedShape {
        let (round, square) = (CornerSize::px(20.0), CornerSize::px(0.0));
        RoundedCornerShape::new(square.clone(), round.clone(), round, square).into()
    }

    /// Compose's `horizontalCenterOptically(shape: CornerBasedShape)`: 0.11 of the start corners' average less the
    /// end corners' (2.2 px, rounded: 2) toward the end, within the offsets allowed; mirrored right to left.
    fn horizontal_center_optically_moves_content_toward_the_rounder_side() {
        let direction = mutable_state_of(LayoutDirection::Ltr);
        let d = direction.clone();
        let rule = KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let d = d.clone();
                KompozitionLocalProvider(&[LocalLayoutDirection.provides(d.get())], move || {
                    Column(
                        content = move || {
                            // (the defaults, 0 each way: nothing moves)
                            Box(Modifier
                                .horizontal_center_optically(rounder_at_the_start())
                                .size_wh(100.dp(), 40.dp())
                                .test_tag("none"));
                            Box(Modifier
                                .horizontal_center_optically_with(rounder_at_the_start(), 0.dp(), 10.dp())
                                .size_wh(100.dp(), 40.dp())
                                .test_tag("end"));
                            Box(Modifier
                                .horizontal_center_optically_with(rounder_at_the_start(), 0.dp(), 1.dp())
                                .size_wh(100.dp(), 40.dp())
                                .test_tag("clamped"));
                        },
                    );
                });
            }),
            IntSize::new(400, 400),
        );
        assert_eq!(rule.density().density, 1.0, "(the px below are at a density of 1)");
        assert_eq!(bounds(&rule, "none").left, 0.0);
        assert_eq!(bounds(&rule, "end").left, 2.0);
        assert_eq!(bounds(&rule, "clamped").left, 1.0);
        direction.set(LayoutDirection::Rtl);
        rule.wait_for_idle();
        // (the Column as wide as the box: right to left the end is to the left)
        assert_eq!(bounds(&rule, "end").left, -2.0);
    }

    /// Compose's internal `rememberAnimatedShape`: its corners (each within half the height) go to the new shape's
    /// along the spec, its optical offset with them; the `ShapeWithHorizontalCenterOptically` overload moves the
    /// content by it.
    fn animated_shapes_animate_their_corners_and_optical_offset() {
        let at_the_end = mutable_state_of(false);
        let shape_seen: Rc<RefCell<Option<Arc<dyn Shape>>>> = Rc::new(RefCell::new(None));
        let (e, seen) = (at_the_end.clone(), shape_seen.clone());
        let rule = KompozTestRule::with_viewport(
            kompozable_lambda!(move || {
                let target = if e.get() { rounder_at_the_end() } else { rounder_at_the_start() };
                let spec = remember(|| {
                    Arc::new(TweenSpec::<f32>::new(100, 0, LinearEasing)) as Arc<dyn FiniteAnimationSpec<f32>>
                });
                let shape = remember_animated_shape(target, spec);
                *seen.borrow_mut() = Some(shape.clone());
                let optical = as_shape_with_horizontal_center_optically(&shape).expect("an animated shape is one");
                Box(named_args!(Modifier.size_wh(100.dp(), 40.dp()).background(Color::Black, shape = shape.clone()))
                    .horizontal_center_optically_of_with(optical, 10.dp(), 10.dp())
                    .background(Color::Red)
                    .test_tag("animated"));
            }),
            IntSize::new(400, 400),
        );
        let offset = || {
            let shape = shape_seen.borrow().clone().unwrap();
            as_shape_with_horizontal_center_optically(&shape).unwrap().offset()
        };
        // Where the content (in red) is drawn: its placement reads the offset, placed again as it changes.
        let content_left = || {
            let canvas = rule.draw();
            let red = canvas.rects.iter().zip(&canvas.rect_colors).find(|(_, c)| **c == Color::Red);
            red.expect("the content drawn").0.left
        };
        // (its corners are made as the outline is: until then each is within 0..1, Compose's first clamped range)
        rule.draw();
        assert!((offset() - 2.2).abs() < 1e-4, "{}", offset());
        rule.wait_for_idle();
        assert_eq!(content_left(), 2.0);
        // To the end's: through the middle, at the end -2.2.
        at_the_end.set(true);
        rule.advance_frame();
        rule.advance_time_by(50);
        rule.draw();
        assert!(offset().abs() < 1.0, "{}", offset());
        rule.advance_time_by(100);
        rule.draw();
        assert!((offset() + 2.2).abs() < 1e-4, "{}", offset());
        rule.wait_for_idle();
        assert_eq!(content_left(), -2.0);
    }

    #[test]
    fn horizontal_center_optically_and_animated_shapes() {
        kompoz_runtime::set_ui_thread();
        horizontal_center_optically_moves_content_toward_the_rounder_side();
        animated_shapes_animate_their_corners_and_optical_offset();
    }
}
