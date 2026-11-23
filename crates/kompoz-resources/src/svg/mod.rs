//! SVG drawables (CMP skiko's SvgPainter.kt and DrawCache.kt, over Skia's SVGDOM): an SVG document read
//! ([`dom`]), painted into the size it is given (its root's width and height set to it, its aspect ratio not kept,
//! as CMP sets them), through a bitmap it keeps and grows (CMP's DrawCache) and draws faded and filtered.

mod dom;

use kompoz_graphics::colorspace::ColorSpaces;
use kompoz_graphics::{
    BlendMode, Color, ColorFilter, Density, ImageBitmap, ImageBitmapConfig, IntOffset, IntSize, LayoutDirection, Rect,
    Size,
};
use kompoz_ui::draw::{Canvas, DrawScope, Fill, Painter};
use std::any::Any;
use std::cell::RefCell;
use std::fmt;

pub(crate) use dom::SvgDom;

/// An SVG that can't be read (Skia's SVGDOM gives no root: its XML, or a root that isn't `<svg>`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SvgParseError(pub String);

impl fmt::Display for SvgParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SvgParseError {}

/// A bitmap drawn into and kept, grown when a bigger size is asked (CMP's `DrawCache`).
struct DrawCache {
    image: Option<ImageBitmap>,
    config: ImageBitmapConfig,
    size: IntSize,
}

impl DrawCache {
    /// `block` drawn into the bitmap's top left `size` (cleared first), the bitmap made anew when it is smaller or
    /// of another config.
    fn draw_cached_image(
        &mut self,
        config: ImageBitmapConfig,
        size: IntSize,
        density: Density,
        layout_direction: LayoutDirection,
        block: impl FnOnce(&mut DrawScope<'_>),
    ) {
        let image = match &self.image {
            Some(i) if size.width <= i.width() && size.height <= i.height() && self.config == config => i.clone(),
            _ => {
                let Some(i) =
                    ImageBitmap::try_new(size.width.max(1), size.height.max(1), config, true, &ColorSpaces::Srgb)
                else {
                    return;
                };
                self.image = Some(i.clone());
                self.config = config;
                i
            }
        };
        self.size = size;
        let mut canvas = Canvas::from_image(&image);
        let mut scope = DrawScope::over_canvas(
            &mut canvas,
            Size::new(size.width as f32, size.height as f32),
            layout_direction,
            density,
        );
        // (CMP's clear(): a black rect in BlendMode.Clear)
        let all = Size::new(image.width() as f32, image.height() as f32);
        scope.draw_rect_with(Color::Black, kompoz_graphics::Offset::Zero, all, 1.0, Fill, None, BlendMode::Clear);
        block(&mut scope);
    }

    /// The bitmap's top left `size` drawn into `target`.
    fn draw_into(&self, target: &mut DrawScope<'_>, alpha: f32, color_filter: Option<&ColorFilter>) {
        let image = self.image.as_ref().expect(
            "drawCachedImage must be invoked first before attempting to draw the result into another destination",
        );
        target.draw_image_rect_with(
            image,
            IntOffset::Zero,
            self.size,
            IntOffset::Zero,
            self.size,
            alpha,
            Fill,
            color_filter.cloned(),
            BlendMode::SrcOver,
            kompoz_graphics::DefaultFilterQuality,
        );
    }
}

/// An SVG drawn into the size it is given (CMP's internal `SvgPainter`): its intrinsic size its root's width and
/// height in px times the density (none if both are 0 or missing); without a `viewBox`, one of that size.
pub struct SvgPainter {
    dom: SvgDom,
    default_size_px: Size,
    density: Density,
    cache: RefCell<DrawCache>,
}

impl SvgPainter {
    /// The SVG of `bytes` at `density`.
    pub fn decode(bytes: &[u8], density: Density) -> Result<SvgPainter, SvgParseError> {
        let mut dom = SvgDom::parse(bytes)?;
        let (w, h) = dom.root_size_px();
        let (w, h) = (w.unwrap_or(0.0), h.unwrap_or(0.0));
        let default_size_px = if w == 0.0 && h == 0.0 { Size::Unspecified } else { Size::new(w, h) };
        if !dom.has_view_box() && default_size_px.is_specified() {
            dom.set_view_box(Rect::new(0.0, 0.0, w, h));
        }
        Ok(SvgPainter {
            dom,
            default_size_px,
            density,
            cache: RefCell::new(DrawCache { image: None, config: ImageBitmapConfig::Argb8888, size: IntSize::Zero }),
        })
    }
}

impl Painter for SvgPainter {
    fn intrinsic_size(&self) -> Size {
        if self.default_size_px.is_specified() {
            Size::new(
                self.default_size_px.width * self.density.density,
                self.default_size_px.height * self.density.density,
            )
        } else {
            Size::Unspecified
        }
    }
    fn on_draw(&self, scope: &mut DrawScope<'_>, alpha: f32, color_filter: Option<&ColorFilter>) {
        let size = scope.size();
        // (CMP compares with a `previousDrawSize` it never sets: it draws the SVG again each time)
        let int_size = IntSize::new(size.width.ceil() as i32, size.height.ceil() as i32);
        if int_size.width <= 0 || int_size.height <= 0 {
            return;
        }
        let density = scope.density();
        let direction = scope.layout_direction();
        let dom = &self.dom;
        self.cache.borrow_mut().draw_cached_image(ImageBitmapConfig::Argb8888, int_size, density, direction, |s| {
            dom.render(s, size.width, size.height);
        });
        self.cache.borrow().draw_into(scope, alpha, color_filter);
    }
    fn applies_alpha(&self) -> bool {
        true
    }
    fn applies_color_filter(&self) -> bool {
        true
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
