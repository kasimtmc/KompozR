//! Material Expressive's loading indicators, as Compose's LoadingIndicator.kt (material3 1.5): a shape that morphs
//! between Material's shapes — by a progress ([`LoadingIndicator`], [`ContainedLoadingIndicator`]: from a circle
//! to a soft burst, turning half way round back) or on its own as long as it is shown
//! ([`IndeterminateLoadingIndicator`], [`IndeterminateContainedLoadingIndicator`]: seven shapes in turn, each morph
//! springing in, a quarter turn more each, on top of a turn every 4.7 s) —, the contained ones in a colored
//! container; [`LoadingIndicatorDefaults`].
//!
//! The shapes are [`RoundedPolygon`]s (kompoz-graphics' MaterialShapes), morphed by [`Morph`]s of their normalized
//! forms, drawn 38 dp across in a 48 dp container. What they tell accessibility is a progress in 0..1 (an endless
//! one: an indeterminate one).

use crate::internal::shape_util::ClipShape;
use crate::internal::shape_util::RoundedPolygonPaths;
use crate::progress_indicator::fast_coerce_in;
use crate::tokens::LoadingIndicatorTokens;
use kompoz_animation::{
    Animatable, AnimationEndReason, AnimationSpec, AnimationVector1D, LinearEasing, RepeatMode, infinite_repeatable,
    spring, tween,
};
use kompoz_coroutines::{CoroutineScope, LaunchedEffect, coroutine_scope, current_coroutine_context, delay};
use kompoz_foundation::prelude::*;
use kompoz_graphics::material_shapes::{MaterialShapes, Morph, RoundedPolygon};
use kompoz_graphics::{Color, Dp, Fill, Matrix, Path, Shape, Size};
use kompoz_runtime::{
    MutableState, kompozable, mutable_float_state_of, mutable_int_state_of, named_args, remember, remember_keyed,
};
use kompoz_ui::semantics::ProgressBarRangeInfo;
use kompoz_ui::{Alignment, InfiniteAnimationPolicy, Modifier};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

// ---- LoadingIndicator ---------------------------------------------------------------------------------------------

/// A determinate loading indicator (Compose's `LoadingIndicator(progress: () -> Float, modifier, color,
/// polygons)`): a `color` shape morphing through `polygons` (at least two: panics otherwise, Kotlin's
/// IllegalArgumentException) by `progress()` (read as it draws, kept in 0..1; NaN told to accessibility as 0), turning
/// half way round back as it goes from 0 to 1. 48 dp unless the modifier sizes it, the shape 38 dp across in it.
///
/// Kotlin: `LoadingIndicator(progress: () -> Float, modifier: Modifier, color: Color, polygons: List<RoundedPolygon>)`
#[kompozable]
pub fn LoadingIndicator(
    progress: impl Fn() -> f32 + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(LoadingIndicatorDefaults::indicator_color())] color: Color,
    #[default(LoadingIndicatorDefaults::DeterminateIndicatorPolygons())] polygons: Vec<RoundedPolygon>,
) {
    DeterminateLoadingIndicatorImpl(
        Rc::new(progress),
        modifier,
        Color::Unspecified,
        color,
        LoadingIndicatorDefaults::container_shape(),
        polygons,
    );
}

/// An indeterminate loading indicator (Compose's `LoadingIndicator(modifier, color, polygons)`): a `color` shape
/// morphing through `polygons` (at least two: panics otherwise, Kotlin's IllegalArgumentException) and back to the
/// first for as long as it is shown — each morph a spring every 650 ms turning it a quarter more, on top of a whole
/// turn every 4.666 s. It tells accessibility an indeterminate progress.
///
/// Kotlin: `LoadingIndicator(modifier: Modifier, color: Color, polygons: List<RoundedPolygon>)`
#[kompozable]
pub fn IndeterminateLoadingIndicator(
    #[default(Modifier)] modifier: Modifier,
    #[default(LoadingIndicatorDefaults::indicator_color())] color: Color,
    #[default(LoadingIndicatorDefaults::IndeterminateIndicatorPolygons())] polygons: Vec<RoundedPolygon>,
) {
    IndeterminateLoadingIndicatorImpl(
        modifier,
        Color::Unspecified,
        color,
        LoadingIndicatorDefaults::container_shape(),
        polygons,
    );
}

/// A determinate contained loading indicator (Compose's `ContainedLoadingIndicator(progress: () -> Float, modifier,
/// containerColor, indicatorColor, containerShape, polygons)`): a [`LoadingIndicator`] of `indicator_color` in a
/// `container_color` container cut to `container_shape` (a circle by default).
///
/// Kotlin: `ContainedLoadingIndicator(progress: () -> Float, modifier: Modifier, containerColor: Color, indicatorColor: Color, containerShape: Shape, polygons: List<RoundedPolygon>)`
#[kompozable]
pub fn ContainedLoadingIndicator(
    progress: impl Fn() -> f32 + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(LoadingIndicatorDefaults::contained_container_color())] container_color: Color,
    #[default(LoadingIndicatorDefaults::contained_indicator_color())] indicator_color: Color,
    #[default(LoadingIndicatorDefaults::container_shape())] container_shape: Arc<dyn Shape>,
    #[default(LoadingIndicatorDefaults::DeterminateIndicatorPolygons())] polygons: Vec<RoundedPolygon>,
) {
    DeterminateLoadingIndicatorImpl(
        Rc::new(progress),
        modifier,
        container_color,
        indicator_color,
        container_shape,
        polygons,
    );
}

/// An indeterminate contained loading indicator (Compose's `ContainedLoadingIndicator(modifier, containerColor,
/// indicatorColor, containerShape, polygons)`): an [`IndeterminateLoadingIndicator`] of `indicator_color` in a
/// `container_color` container cut to `container_shape` (a circle by default).
///
/// Kotlin: `ContainedLoadingIndicator(modifier: Modifier, containerColor: Color, indicatorColor: Color, containerShape: Shape, polygons: List<RoundedPolygon>)`
#[kompozable]
pub fn IndeterminateContainedLoadingIndicator(
    #[default(Modifier)] modifier: Modifier,
    #[default(LoadingIndicatorDefaults::contained_container_color())] container_color: Color,
    #[default(LoadingIndicatorDefaults::contained_indicator_color())] indicator_color: Color,
    #[default(LoadingIndicatorDefaults::container_shape())] container_shape: Arc<dyn Shape>,
    #[default(LoadingIndicatorDefaults::IndeterminateIndicatorPolygons())] polygons: Vec<RoundedPolygon>,
) {
    IndeterminateLoadingIndicatorImpl(modifier, container_color, indicator_color, container_shape, polygons);
}

/// Kotlin's `require(indicatorPolygons.size > 1)`.
fn require_two_polygons(indicator_polygons: &[RoundedPolygon]) {
    if indicator_polygons.len() <= 1 {
        kompoz_runtime::kt::panic_kt(
            "IllegalArgumentException",
            "indicatorPolygons should have, at least, two RoundedPolygons",
        );
    }
}

/// A determinate loading indicator implementation (Compose's private `LoadingIndicatorImpl(progress, …)`).
#[kompozable]
fn DeterminateLoadingIndicatorImpl(
    progress: Rc<dyn Fn() -> f32>,
    modifier: Modifier,
    container_color: Color,
    indicator_color: Color,
    container_shape: Arc<dyn Shape>,
    indicator_polygons: Vec<RoundedPolygon>,
) {
    require_two_polygons(&indicator_polygons);
    let coerced_progress = move || fast_coerce_in(progress(), 0.0, 1.0);
    let semantics_progress = coerced_progress.clone();
    let path = remember(|| Rc::new(RefCell::new(Path::new())));
    let scale_matrix = remember(|| Rc::new(RefCell::new(Matrix::new())));
    let morph_sequence =
        remember_keyed(indicator_polygons.clone(), || Rc::new(morph_sequence(&indicator_polygons, false)));
    // (Compose keys this by the morph sequence, which changes exactly when the polygons it is made of do: keyed by
    // them here, a Morph having no equality)
    let morph_scale_factor = remember_keyed(indicator_polygons.clone(), || {
        // Calculate the shapes scale factor that will be applied to the morphed path as it's
        // scaled into the available size.
        // This overall scale factor ensures that the shapes are rendered without clipping and
        // at the correct ratio within the component by taking into account their occupied size
        // as they rotate, and taking into account the spec's ActiveIndicatorScale.
        calculate_scale_factor(&indicator_polygons) * LoadingIndicatorDefaults::ActiveIndicatorScale
    });
    Box(
        modifier = named_args!(
            modifier
                .semantics_with(true, move |c| {
                    // Check for NaN, as the ProgressBarRangeInfo will throw an exception.
                    let current = Some(semantics_progress()).filter(|p| !p.is_nan()).unwrap_or(0.0);
                    c.set_progress_bar_range_info(ProgressBarRangeInfo::new(current, 0.0..=1.0, 0));
                })
                .size_wh(LoadingIndicatorDefaults::ContainerWidth, LoadingIndicatorDefaults::ContainerHeight)
                .fill_max_size()
                .clip(ClipShape(container_shape))
                .background(container_color)
        ),
        content_alignment = Alignment::Center,
        content = move || {
            let (coerced_progress, path, scale_matrix, morph_sequence) =
                (coerced_progress.clone(), path.clone(), scale_matrix.clone(), morph_sequence.clone());
            // Using a Spacer to render the indicator's shapes. Maintains a square aspect ratio (1:1)
            // to prevent shape distortion during rendering.
            Spacer(Modifier.aspect_ratio_with(1.0, true).draw_with_content(move |c| {
                let progress_value = coerced_progress();
                let size = morph_sequence.len();
                // Adjust the active morph index according to the progress.
                let active_morph_index = ((size as f32 * progress_value) as i32).min(size as i32 - 1) as usize;
                // Prepare the progress value that will be used for the active Morph.
                let adjusted_progress_value = if progress_value == 1.0 && active_morph_index == size - 1 {
                    // Prevents a zero when the progress is one and we are at the last
                    // shape morph.
                    1.0
                } else {
                    (progress_value * size as f32) % 1.0
                };

                // Rotate counterclockwise.
                let rotation = -progress_value * 180.0;
                let center = c.center();
                c.rotate_with(rotation, center, |c| {
                    let mut path = path.borrow_mut();
                    // (ShapeUtil's Morph.toPath(progress, path, startAngle = 0): not turned)
                    // Use the adjusted progress.
                    morph_sequence[active_morph_index].to_path_into(
                        adjusted_progress_value,
                        &mut path,
                        0,
                        false,
                        true,
                        0.0,
                        0.0,
                    );
                    process_path(&mut path, c.size(), morph_scale_factor, &mut scale_matrix.borrow_mut());
                    named_args!(c.draw_path(&path, indicator_color, style = Fill));
                });
            }));
        },
    );
}

/// An indeterminate loading indicator implementation (Compose's private `LoadingIndicatorImpl(modifier, …)`).
#[kompozable]
fn IndeterminateLoadingIndicatorImpl(
    modifier: Modifier,
    container_color: Color,
    indicator_color: Color,
    container_shape: Arc<dyn Shape>,
    indicator_polygons: Vec<RoundedPolygon>,
) {
    require_two_polygons(&indicator_polygons);
    let morph_sequence =
        remember_keyed(indicator_polygons.clone(), || Rc::new(morph_sequence(&indicator_polygons, true)));
    let shapes_scale_factor = remember_keyed(indicator_polygons.clone(), || {
        // Calculate the shapes scale factor that will be applied to the morphed path as it's
        // scaled into the available size.
        // This overall scale factor ensures that the shapes are rendered without clipping and
        // at the correct ratio within the component by taking into account their occupied size
        // as they rotate, and taking into account the spec's ActiveIndicatorScale.
        calculate_scale_factor(&indicator_polygons) * LoadingIndicatorDefaults::ActiveIndicatorScale
    });
    let morph_progress: Animatable<f32, AnimationVector1D> = remember(|| Animatable(0.0f32));
    let morph_rotation_target_angle: MutableState<f32> = remember(|| mutable_float_state_of(QuarterRotation));
    let global_rotation: Animatable<f32, AnimationVector1D> = remember(|| Animatable(0.0f32));
    let current_morph_index: MutableState<i32> = remember_keyed(indicator_polygons.clone(), || mutable_int_state_of(0));
    {
        let (morph_progress, morph_rotation_target_angle, global_rotation, current_morph_index, morph_sequence) = (
            morph_progress.clone(),
            morph_rotation_target_angle.clone(),
            global_rotation.clone(),
            current_morph_index.clone(),
            morph_sequence.clone(),
        );
        LaunchedEffect(indicator_polygons.clone(), async move {
            let morph_animation_block = move |scope: &CoroutineScope| {
                let (morph_progress, morph_rotation_target_angle, current_morph_index, morph_sequence) = (
                    morph_progress.clone(),
                    morph_rotation_target_angle.clone(),
                    current_morph_index.clone(),
                    morph_sequence.clone(),
                );
                named_args!(scope.launch(async move {
                    // Note that we up the visibilityThreshold here to 0.1, which is x10 than the
                    // default threshold, and ends the low-damping spring in a shorter time.
                    let morph_animation_spec: Arc<dyn AnimationSpec<f32>> = Arc::new(named_args!(spring(
                        damping_ratio = 0.6,
                        stiffness = 200.0,
                        visibility_threshold = Some(0.1f32)
                    )));
                    // (the launched coroutine's own scope, what Kotlin's `async` there is a child of)
                    coroutine_scope(|this| async move {
                        loop {
                            // Async launch of a spring that will finish in less than 650ms
                            // (MorphIntervalMillis). We then delay the entire while loop by 650ms till the
                            // next morph starts.
                            let (morph_progress, morph_rotation_target_angle, current_morph_index, morph_sequence) = (
                                morph_progress.clone(),
                                morph_rotation_target_angle.clone(),
                                current_morph_index.clone(),
                                morph_sequence.clone(),
                            );
                            let morph_animation_spec = morph_animation_spec.clone();
                            let deferred = named_args!(this.r#async(async move {
                                let animation_result =
                                    named_args!(morph_progress.animate_to(1.0, animation_spec = morph_animation_spec))
                                        .await;
                                if animation_result.end_reason == AnimationEndReason::Finished {
                                    current_morph_index
                                        .set((current_morph_index.get() + 1) % morph_sequence.len() as i32);
                                    morph_progress.snap_to(0.0).await;
                                    morph_rotation_target_angle
                                        .set((morph_rotation_target_angle.get() + QuarterRotation) % FullRotation);
                                }
                            }));
                            delay(MorphIntervalMillis).await;
                            deferred.r#await().await;
                        }
                    })
                    .await
                }))
            };

            let rotation_animation_block = move |scope: &CoroutineScope| {
                let global_rotation = global_rotation.clone();
                named_args!(scope.launch(async move {
                    let animation_spec: Arc<dyn AnimationSpec<f32>> = Arc::new(named_args!(infinite_repeatable(
                        named_args!(tween(GlobalRotationDurationMillis, easing = Arc::new(LinearEasing))),
                        repeat_mode = RepeatMode::Restart
                    )));
                    named_args!(global_rotation.animate_to(FullRotation, animation_spec = animation_spec)).await;
                }))
            };

            // (the effect's coroutine scope, what Kotlin's `launch` here is called on)
            coroutine_scope(|scope| async move {
                // Possibly skip the infinite animation block when an InfiniteAnimationPolicy is
                // installed.
                match current_coroutine_context().get::<InfiniteAnimationPolicy>() {
                    None => {
                        morph_animation_block(&scope);
                        rotation_animation_block(&scope);
                    }
                    Some(policy) => {
                        policy
                            .on_infinite_operation(async move {
                                morph_animation_block(&scope);
                                rotation_animation_block(&scope);
                            })
                            .await
                    }
                }
            })
            .await;
        });
    }

    let path = remember(|| Rc::new(RefCell::new(Path::new())));
    let scale_matrix = remember(|| Rc::new(RefCell::new(Matrix::new())));
    Box(
        modifier = named_args!(
            modifier
                .progress_semantics_indeterminate()
                .size_wh(LoadingIndicatorDefaults::ContainerWidth, LoadingIndicatorDefaults::ContainerHeight)
                .fill_max_size()
                .clip(ClipShape(container_shape))
                .background(container_color)
        ),
        content_alignment = Alignment::Center,
        content = move || {
            let (morph_progress, morph_rotation_target_angle, global_rotation, current_morph_index) = (
                morph_progress.clone(),
                morph_rotation_target_angle.clone(),
                global_rotation.clone(),
                current_morph_index.clone(),
            );
            let (morph_sequence, path, scale_matrix) = (morph_sequence.clone(), path.clone(), scale_matrix.clone());
            Spacer(Modifier.aspect_ratio_with(1.0, true).draw_with_content(move |c| {
                let progress = morph_progress.value();
                // Rotate clockwise.
                let rotation = progress * 90.0 + morph_rotation_target_angle.get() + global_rotation.value();
                let center = c.center();
                c.rotate_with(rotation, center, |c| {
                    let mut path = path.borrow_mut();
                    // (ShapeUtil's Morph.toPath(progress, path, startAngle = 0): not turned)
                    // Use the coerced progress value to eliminate any
                    // bounciness from the morph. We scale the drawing
                    // to simulate some bounciness instead.
                    morph_sequence[current_morph_index.get() as usize]
                        .to_path_into(progress, &mut path, 0, false, true, 0.0, 0.0);
                    process_path(&mut path, c.size(), shapes_scale_factor, &mut scale_matrix.borrow_mut());
                    named_args!(c.draw_path(&path, indicator_color, style = Fill));
                });
            }));
        },
    );
}

// ---- LoadingIndicatorDefaults -------------------------------------------------------------------------------------

/// The loading indicators' defaults (Compose's `LoadingIndicatorDefaults`): its properties associated items.
pub struct LoadingIndicatorDefaults;

#[allow(non_snake_case)]
impl LoadingIndicatorDefaults {
    /// A [`LoadingIndicator`] default container width: 48 dp.
    pub const ContainerWidth: Dp = LoadingIndicatorTokens::ContainerWidth;

    /// A [`LoadingIndicator`] default container height: 48 dp.
    pub const ContainerHeight: Dp = LoadingIndicatorTokens::ContainerHeight;

    /// A [`LoadingIndicator`] default active indicator size: 38 dp.
    pub const IndicatorSize: Dp = LoadingIndicatorTokens::ActiveSize;

    /// A [`LoadingIndicator`] default container [`Shape`]: the scheme's full corners, a circle (Compose's
    /// `containerShape`).
    pub fn container_shape() -> Arc<dyn Shape> {
        LoadingIndicatorTokens::ContainerShape.value()
    }

    /// A [`LoadingIndicator`] default active indicator color when using an uncontained [`LoadingIndicator`]: the
    /// scheme's primary (Compose's `indicatorColor`).
    pub fn indicator_color() -> Color {
        LoadingIndicatorTokens::ActiveIndicatorColor.value()
    }

    /// A [`LoadingIndicator`] default active indicator color when using a [`ContainedLoadingIndicator`]: the
    /// scheme's on primary container (Compose's `containedIndicatorColor`).
    pub fn contained_indicator_color() -> Color {
        LoadingIndicatorTokens::ContainedActiveColor.value()
    }

    /// A [`LoadingIndicator`] default container color when using a [`ContainedLoadingIndicator`]: the scheme's
    /// primary container (Compose's `containedContainerColor`).
    pub fn contained_container_color() -> Color {
        LoadingIndicatorTokens::ContainedContainerColor.value()
    }

    /// The sequence of [`RoundedPolygon`]s that the indeterminate [`LoadingIndicator`] will morph between when
    /// animating (Compose's `IndeterminateIndicatorPolygons`): by default seven shapes — SoftBurst, Cookie9Sided,
    /// Pentagon, Pill, Sunny, Cookie4Sided, Oval —, but you may provide your own shapes sequence when calling the
    /// API.
    pub fn IndeterminateIndicatorPolygons() -> Vec<RoundedPolygon> {
        static POLYGONS: OnceLock<Vec<RoundedPolygon>> = OnceLock::new();
        POLYGONS
            .get_or_init(|| {
                vec![
                    MaterialShapes::SoftBurst().clone(),
                    MaterialShapes::Cookie9Sided().clone(),
                    MaterialShapes::Pentagon().clone(),
                    MaterialShapes::Pill().clone(),
                    MaterialShapes::Sunny().clone(),
                    MaterialShapes::Cookie4Sided().clone(),
                    MaterialShapes::Oval().clone(),
                ]
            })
            .clone()
    }

    /// The sequence of [`RoundedPolygon`]s that the determinate [`LoadingIndicator`] will morph between when
    /// animating (Compose's `DeterminateIndicatorPolygons`): it morphs between the shapes as its progress moves
    /// between zero to one; by default two shapes, a circle (turned a twentieth) and SoftBurst, but you may provide
    /// your own shapes sequence when calling the API.
    pub fn DeterminateIndicatorPolygons() -> Vec<RoundedPolygon> {
        static POLYGONS: OnceLock<Vec<RoundedPolygon>> = OnceLock::new();
        POLYGONS
            .get_or_init(|| {
                let mut rotation = Matrix::new();
                rotation.rotate_z(360.0 / 20.0);
                vec![
                    // Rotating the circle gets us a smoother morphing to the soft-burst shapes, which is
                    // also being rotated at the same angle.
                    MaterialShapes::Circle().transformed_by_matrix(&rotation),
                    MaterialShapes::SoftBurst().clone(),
                ]
            })
            .clone()
    }

    /// A percentage value of the LoadingIndicator's active indicator scale when rendered within the bounds of the
    /// container size (Compose's internal `ActiveIndicatorScale`).
    pub(crate) const ActiveIndicatorScale: f32 = LoadingIndicatorDefaults::IndicatorSize.0
        / f32::min(LoadingIndicatorDefaults::ContainerWidth.0, LoadingIndicatorDefaults::ContainerHeight.0);
}

// ---- Morphs, scales and paths -------------------------------------------------------------------------------------

/// The [`Morph`]s that describe the sequence of shapes that the loading indicator should animate in a loop
/// (Compose's private `morphSequence`): one between each consecutive polygon, and with `circular_sequence` a final
/// one from the last shape to the first. Each polygon within the morphs is normalized here.
fn morph_sequence(polygons: &[RoundedPolygon], circular_sequence: bool) -> Vec<Morph> {
    let mut morphs = Vec::with_capacity(polygons.len());
    for (i, polygon) in polygons.iter().enumerate() {
        if let Some(next) = polygons.get(i + 1) {
            morphs.push(Morph::new(polygon.normalized(), next.normalized()));
        } else if circular_sequence {
            // Create a morph from the last shape to the first shape
            morphs.push(Morph::new(polygon.normalized(), polygons[0].normalized()));
        }
    }
    morphs
}

/// A scale factor that will be used when scaling the polygons into a specified sized container (Compose's private
/// `calculateScaleFactor`).
///
/// Since the polygons may rotate, a simple bounds calculation is not enough to determine the size the polygon will
/// occupy as it rotates (it may result in a clipped shape): this compares each polygon's max bounds (its extent at
/// any rotation) to its bounds.
fn calculate_scale_factor(indicator_polygons: &[RoundedPolygon]) -> f32 {
    let mut scale_factor = 1.0f32;
    // Axis-aligned max bounding box for this object, where the rectangles left, top, right, and
    // bottom values will be stored in entries 0, 1, 2, and 3, in that order.
    for polygon in indicator_polygons {
        let bounds = polygon.calculate_bounds(true);
        let max_bounds = polygon.calculate_max_bounds();
        let scale_x = width(&bounds) / width(&max_bounds);
        let scale_y = height(&bounds) / height(&max_bounds);
        // We use max(scaleX, scaleY) to handle cases like a pill-shape that can throw off the
        // entire calculation.
        scale_factor = scale_factor.min(scale_x.max(scale_y));
    }
    scale_factor
}

/// The width of bounds `[left, top, right, bottom]` (Compose's private `FloatArray.width()`).
fn width(bounds: &[f32; 4]) -> f32 {
    bounds[2] - bounds[0]
}

/// The height of bounds `[left, top, right, bottom]` (Compose's private `FloatArray.height()`).
fn height(bounds: &[f32; 4]) -> f32 {
    bounds[3] - bounds[1]
}

/// Scales a path of a normalized morph or polygon uniformly by `size` times `scale_factor` and centers it in `size`
/// (Compose's private `processPath`); `scale_matrix` is reset and used for the scaling.
fn process_path(path: &mut Path, size: Size, scale_factor: f32, scale_matrix: &mut Matrix) {
    scale_matrix.reset();

    scale_matrix.scale(size.width * scale_factor, size.height * scale_factor, 1.0);

    // Scale to the desired size.
    path.transform(scale_matrix);

    // Translate the path to align its center with the available size center.
    let bounds = path.get_bounds();
    path.translate(size.center() - bounds.center());
}

const GlobalRotationDurationMillis: i32 = 4666;
const MorphIntervalMillis: i64 = 650;

const FullRotation: f32 = 360.0;
const QuarterRotation: f32 = FullRotation / 4.0;
