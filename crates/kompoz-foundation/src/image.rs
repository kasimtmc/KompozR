//! `Image`, as Compose's: a painter (a bitmap's, a vector's, any other) in a layout it sizes, scaled and placed in
//! it, cut to its bounds; what a screen reader says of it.
//!
//! Compose's three overloads are three composables here: [`Image`] (a painter), [`BitmapImage`] and [`VectorImage`].

use kompoz_graphics::{ColorFilter, ContentScale, DefaultFilterQuality, FilterQuality, ImageBitmap, ImageVector};
use kompoz_runtime::{kompozable, remember_keyed};
use kompoz_ui::draw::{BitmapPainter, Painter, remember_vector_painter};
use kompoz_ui::layout::layout_into;
use kompoz_ui::semantics::Role;
use kompoz_ui::{Alignment, Constraints, Measurable, MeasurePolicy, MeasureResult, MeasureScope, Modifier};
use std::rc::Rc;

/// `painter` drawn in a layout it sizes (to its intrinsic size, scaled by `content_scale` to what the layout may
/// be), placed by `alignment`, faded by `alpha`, through `color_filter`, cut to its bounds (Compose's
/// `Image(painter, …)`); `content_description` is what a screen reader says of it (none: it is decorative).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn Image(
    painter: Rc<dyn Painter>,
    content_description: Option<String>,
    #[default(Modifier)] modifier: Modifier,
    #[default(Alignment::Center)] alignment: Alignment,
    #[default(ContentScale::Fit)] content_scale: ContentScale,
    #[default(1.0)] alpha: f32,
    #[default(None)] color_filter: Option<ColorFilter>,
) {
    image_layout(painter, content_description, modifier, alignment, content_scale, alpha, color_filter);
}

/// A bitmap as an [`Image`] (Compose's `Image(bitmap, …)`), sampled as `filter_quality` says.
///
/// Kotlin: `Image(bitmap: ImageBitmap, contentDescription: String?, modifier: Modifier = Modifier, alignment: Alignment = Alignment.Center, contentScale: ContentScale = ContentScale.Fit, alpha: Float = DefaultAlpha, colorFilter: ColorFilter? = null, filterQuality: FilterQuality = DefaultFilterQuality)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn BitmapImage(
    bitmap: ImageBitmap,
    content_description: Option<String>,
    #[default(Modifier)] modifier: Modifier,
    #[default(Alignment::Center)] alignment: Alignment,
    #[default(ContentScale::Fit)] content_scale: ContentScale,
    #[default(1.0)] alpha: f32,
    #[default(None)] color_filter: Option<ColorFilter>,
    #[default(DefaultFilterQuality)] filter_quality: FilterQuality,
) {
    let painter = remember_keyed((bitmap.clone(), filter_quality), move || {
        let p: Rc<dyn Painter> = Rc::new(BitmapPainter::with_quality(bitmap, filter_quality));
        p
    });
    image_layout(painter, content_description, modifier, alignment, content_scale, alpha, color_filter);
}

/// A vector as an [`Image`] (Compose's `Image(imageVector, …)`), in its own colors unless filtered.
///
/// Kotlin: `Image(imageVector: ImageVector, contentDescription: String?, modifier: Modifier = Modifier, alignment: Alignment = Alignment.Center, contentScale: ContentScale = ContentScale.Fit, alpha: Float = DefaultAlpha, colorFilter: ColorFilter? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn VectorImage(
    image_vector: ImageVector,
    content_description: Option<String>,
    #[default(Modifier)] modifier: Modifier,
    #[default(Alignment::Center)] alignment: Alignment,
    #[default(ContentScale::Fit)] content_scale: ContentScale,
    #[default(1.0)] alpha: f32,
    #[default(None)] color_filter: Option<ColorFilter>,
) {
    let painter: Rc<dyn Painter> = Rc::new(remember_vector_painter(image_vector));
    image_layout(painter, content_description, modifier, alignment, content_scale, alpha, color_filter);
}

/// Compose's Image body: the description's semantics (an image to accessibility), cut to its bounds, the painter
/// painted, measured as small as it may be.
#[track_caller]
fn image_layout(
    painter: Rc<dyn Painter>,
    content_description: Option<String>,
    modifier: Modifier,
    alignment: Alignment,
    content_scale: ContentScale,
    alpha: f32,
    color_filter: Option<ColorFilter>,
) {
    let described = match content_description {
        Some(description) => modifier.semantics(move |c| {
            c.set_content_description(description.clone());
            c.set_role(Role::Image);
        }),
        None => modifier,
    };
    let m = described.clip_to_bounds().paint_with(painter, true, alignment, content_scale, alpha, color_filter);
    layout_into(m, EMPTY_MEASURE_POLICY.with(Rc::clone), false, || {});
}

thread_local! {
    /// Compose's EmptyMeasurePolicy, shared.
    static EMPTY_MEASURE_POLICY: Rc<dyn MeasurePolicy> = Rc::new(EmptyMeasurePolicy);
}

/// Compose's Image's EmptyMeasurePolicy: as small as it may be.
struct EmptyMeasurePolicy;

impl MeasurePolicy for EmptyMeasurePolicy {
    fn measure(&self, scope: &MeasureScope, _: &[&dyn Measurable], constraints: Constraints) -> MeasureResult {
        scope.layout(constraints.min_width, constraints.min_height, |_| {})
    }
}
