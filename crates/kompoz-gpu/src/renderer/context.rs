//! The GraphicsContext the app draws through (C++'s Renderer's overrides), each call handed to the part of the
//! renderer that draws it.

use super::{Renderer, clip, effect, image, layer, mesh, path, prims, shadow, text};
use kompoz_graphics::{
    BlendMode, Brush, Color, ColorFilter, FilterQuality, ImageBitmap, Matrix, Offset, Outline, Path, PathStroke, Rect,
    RenderEffect, Shadow, Vertices,
};
use kompoz_ui::graphics_context::{
    ClipOp, CornerRadii, ElevationShadow, GraphicsContext, LayerContent, LayerTransform, ShadowLight, TextDraw,
};

impl GraphicsContext for Renderer {
    fn fill_round_rect(&mut self, rect: Rect, radii: CornerRadii, color: Color) {
        prims::fill_round_rect(self, rect, radii, color);
    }

    fn stroke_round_rect(&mut self, rect: Rect, radii: CornerRadii, width: f32, color: Color) {
        prims::stroke_round_rect(self, rect, radii, width, color);
    }

    fn draw_line(&mut self, from: Offset, to: Offset, width: f32, color: Color) {
        prims::draw_line(self, from, to, width, color);
    }

    fn draw_text(&mut self, text: &TextDraw, color: Color) {
        text::draw_text(self, text, color);
    }

    fn draw_text_brush(&mut self, text: &TextDraw, brush: &Brush, brush_area: Rect) {
        text::draw_text_brush(self, text, brush, brush_area);
    }

    fn draw_text_shadow(&mut self, text: &TextDraw, color: Color, blur_radius: f32) {
        text::draw_text_shadow(self, text, color, blur_radius);
    }

    fn draw_arc(
        &mut self,
        oval: Rect,
        start_degrees: f32,
        sweep_degrees: f32,
        stroke_width: f32,
        color: Color,
        round_cap: bool,
    ) {
        prims::draw_arc(self, oval, start_degrees, sweep_degrees, stroke_width, color, round_cap);
    }

    fn draw_image(&mut self, image: &ImageBitmap, src: Rect, dst: Rect, alpha: f32, quality: FilterQuality) {
        image::draw_image(self, image, src, dst, alpha, quality);
    }

    fn draw_image_mask(
        &mut self,
        image: &ImageBitmap,
        src: Rect,
        dst: Rect,
        brush: &Brush,
        brush_area: Rect,
        alpha: f32,
        quality: FilterQuality,
    ) {
        image::draw_image_mask(self, image, src, dst, brush, brush_area, alpha, quality);
    }

    fn push_layer(&mut self, bounds: Rect, radii: CornerRadii, alpha: f32, transform: LayerTransform) {
        layer::push_layer(self, bounds, radii, alpha, transform);
    }

    fn pop_layer(&mut self) {
        layer::pop_layer(self);
    }

    fn push_clip(&mut self, rect: Rect) {
        clip::push_clip(self, rect);
    }

    fn push_clip_round_rect(&mut self, rect: Rect, radii: CornerRadii, op: ClipOp) {
        clip::push_clip_round_rect(self, rect, radii, op);
    }

    fn push_clip_path(&mut self, path: &Path, origin: Offset, op: ClipOp) {
        clip::push_clip_path(self, path, origin, op);
    }

    fn pop_clip(&mut self) {
        clip::pop_clip(self);
    }

    fn push_translation(&mut self, by: Offset) {
        super::push_translation(self, by);
    }

    fn pop_translation(&mut self) {
        super::pop_translation(self);
    }

    fn push_transform(&mut self, matrix: &Matrix) {
        super::push_transform(self, matrix);
    }

    fn pop_transform(&mut self) {
        super::pop_translation(self);
    }

    fn push_effect_layer(&mut self, bounds: Rect, effect: &RenderEffect, alpha: f32) {
        effect::push_effect_layer(self, bounds, effect, alpha);
    }

    fn draw_backdrop(&mut self, bounds: Rect, effect: &RenderEffect, alpha: f32) {
        effect::draw_backdrop(self, bounds, effect, alpha);
    }

    fn push_blend_mode(&mut self, mode: BlendMode) {
        super::push_blend_mode(self, mode);
    }

    fn pop_blend_mode(&mut self) {
        super::pop_blend_mode(self);
    }

    fn push_anti_alias(&mut self, on: bool) {
        super::push_anti_alias(self, on);
    }

    fn pop_anti_alias(&mut self) {
        super::pop_anti_alias(self);
    }

    fn push_color_filter(&mut self, filter: &ColorFilter) {
        super::push_color_filter(self, filter);
    }

    fn pop_color_filter(&mut self) {
        super::pop_color_filter(self);
    }

    fn quick_reject(&self, rect: Rect) -> bool {
        super::quick_reject(self, rect)
    }

    fn fill_gradient(&mut self, rect: Rect, radii: CornerRadii, brush: &Brush, brush_area: Rect) {
        prims::fill_gradient(self, rect, radii, brush, brush_area);
    }

    fn draw_vertices(
        &mut self,
        vertices: &Vertices,
        origin: Offset,
        blend_mode: BlendMode,
        brush: &Brush,
        brush_area: Rect,
        alpha: f32,
    ) {
        mesh::draw_vertices(self, vertices, origin, blend_mode, brush, brush_area, alpha);
    }

    fn draw_path(&mut self, path: &Path, origin: Offset, brush: &Brush, brush_area: Rect, stroke: &PathStroke) {
        path::draw_path(self, path, origin, brush, brush_area, stroke);
    }

    fn draw_shadow(&mut self, rect: Rect, radii: CornerRadii, shadow: &Shadow, inner: bool) {
        prims::draw_shadow(self, rect, radii, shadow, inner);
    }

    fn draw_shadow_path(&mut self, outline: &Path, origin: Offset, shadow: &Shadow, inner: bool) {
        shadow::draw_shadow_path(self, outline, origin, shadow, inner);
    }

    fn light(&self) -> ShadowLight {
        shadow::light(self)
    }

    fn light_center(&self) -> Offset {
        shadow::light_center(self)
    }

    fn draw_elevation_shadow(&mut self, outline: &Outline, origin: Offset, shadow: &ElevationShadow) {
        shadow::draw_elevation_shadow(self, outline, origin, shadow);
    }

    fn push_outline_layer(&mut self, bounds: Rect, outline: &Path, alpha: f32, transform: LayerTransform) {
        layer::push_outline_layer(self, bounds, outline, alpha, transform);
    }

    fn key_layer(&mut self, key: u64, same: bool) {
        layer::key_layer(self, key, same);
    }

    fn layer_content(&self) -> LayerContent {
        layer::layer_content(self)
    }
}
