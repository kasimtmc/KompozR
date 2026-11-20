//! Material's Icon, as Compose's Icon.kt: a painter (a vector's, a bitmap's, any) drawn fit in a layout of its own
//! intrinsic size (24 dp where it has none), tinted the content color around ([`LocalContentColor`]) unless told
//! otherwise, an image to accessibility when described.
//!
//! Compose's overloads are composables of their own here: [`Icon`] (a painter), [`VectorIcon`] (an `ImageVector`,
//! Kotlin's `Icon(imageVector, …)`), [`BitmapIcon`] (an `ImageBitmap`) and [`TintProducerIcon`] (a painter tinted
//! by a color read as it draws).

use crate::content_color::LocalContentColor;
use crate::tokens::SmallIconButtonTokens;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{BlendMode, Color, ColorFilter, ColorProducer, ContentScale, ImageBitmap, ImageVector};
use kompoz_runtime::{kompozable, named_args, remember_keyed};
use kompoz_ui::Modifier;
use kompoz_ui::draw::{BitmapPainter, Painter, remember_vector_painter};
use kompoz_ui::semantics::Role;
use std::rc::Rc;

/// `image_vector` as an icon (Compose's `Icon(imageVector, …)`): see [`Icon`].
///
/// Kotlin: `Icon(imageVector: ImageVector, contentDescription: String?)`
#[kompozable]
pub fn VectorIcon(
    image_vector: ImageVector,
    content_description: Option<String>,
    #[default(Modifier)] modifier: Modifier,
    #[default(LocalContentColor.current())] tint: Color,
) {
    let painter: Rc<dyn Painter> = Rc::new(remember_vector_painter(image_vector));
    icon(painter, content_description, modifier, tint);
}

/// `bitmap` as an icon (Compose's `Icon(bitmap, …)`): see [`Icon`].
///
/// Kotlin: `Icon(bitmap: ImageBitmap, contentDescription: String?)`
#[kompozable]
pub fn BitmapIcon(
    bitmap: ImageBitmap,
    content_description: Option<String>,
    #[default(Modifier)] modifier: Modifier,
    #[default(LocalContentColor.current())] tint: Color,
) {
    let painter = remember_keyed(bitmap.clone(), move || {
        let painter: Rc<dyn Painter> =
            Rc::new(BitmapPainter::with_quality(bitmap, kompoz_graphics::DefaultFilterQuality));
        painter
    });
    icon(painter, content_description, modifier, tint);
}

/// An icon (Compose's `Icon(painter, …)`): `painter` drawn fit (its aspect kept) in a layout of its intrinsic size
/// (24 dp each way where it has none), tinted `tint` (the content color around by default; unspecified: in its own
/// colors), an image to accessibility described as `content_description` (none: decorative).
#[kompozable]
pub fn Icon(
    painter: Rc<dyn Painter>,
    content_description: Option<String>,
    #[default(Modifier)] modifier: Modifier,
    #[default(LocalContentColor.current())] tint: Color,
) {
    icon(painter, content_description, modifier, tint);
}

/// An icon tinted by what `tint` gives as it draws (Compose's `Icon(painter, tint: ColorProducer?, …)`): an
/// animated tint redraws it without composing it again; none: in the painter's own colors. The rest as [`Icon`]'s.
///
/// Kotlin: `Icon(painter: Painter, tint: ColorProducer?, contentDescription: String?)`
#[kompozable]
pub fn TintProducerIcon(
    painter: Rc<dyn Painter>,
    tint: Option<ColorProducer>,
    content_description: Option<String>,
    #[default(Modifier)] modifier: Modifier,
) {
    let modifier = modifier.draw_with_cache(move |scope| {
        let layer = scope.obtain_graphics_layer();
        scope.record(&layer, |s| s.draw_content());
        if let Some(tint) = &tint {
            layer.set_color_filter(Some(ColorFilter::tint(tint.get(), BlendMode::SrcIn)));
        }
        scope.on_draw_with_content(move |s| s.draw_layer(&layer))
    });
    icon(painter, content_description, modifier, Color::Unspecified);
}

/// Compose's `Icon(painter, …)` body: the description's semantics, the painter painted fit, sized as it is or 24 dp,
/// in an empty Box.
#[kompozable(inline)]
fn icon(painter: Rc<dyn Painter>, content_description: Option<String>, modifier: Modifier, tint: Color) {
    let color_filter = remember_keyed(tint, || {
        if tint == Color::Unspecified { None } else { Some(ColorFilter::tint(tint, BlendMode::SrcIn)) }
    });
    let semantics = match content_description {
        Some(description) => Modifier.semantics(move |s| {
            s.set_content_description(description.clone());
            s.set_role(Role::Image);
        }),
        None => Modifier,
    };
    // (Compose's toolingGraphicsLayer: a layer only while debug inspector info is on, which it isn't)
    let sized = default_size_for(modifier, &*painter);
    let modifier = named_args!(sized.paint(painter, color_filter = color_filter, content_scale = ContentScale::Fit))
        .then(semantics);
    Box(modifier = modifier);
}

/// The layout 24 dp each way where `painter` has no size of its own (Compose's `defaultSizeFor`).
fn default_size_for(modifier: Modifier, painter: &dyn Painter) -> Modifier {
    let size = painter.intrinsic_size();
    // (Kotlin's `== Size.Unspecified` compares bits: NaN is NaN there)
    if size.is_unspecified() || (size.width.is_infinite() && size.height.is_infinite()) {
        modifier.size(SmallIconButtonTokens::IconSize)
    } else {
        modifier
    }
}
