//! Passes: a render encoder each, how it starts (cleared, loaded, or not loaded at all), its scissors kept within its
//! area, uniforms, textures with their samplers, and instanced draws.

use super::MetalDevice;
use crate::device::{
    BufferRef, ClearColor, Filter, Format, IRect, MAX_TEXTURES, MAX_UNIFORMS, Sampler, TextureId, Wrap,
};
use objc2::Message;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{
    MTLClearColor, MTLCommandBuffer, MTLCommandEncoder, MTLDevice, MTLLoadAction, MTLPrimitiveType,
    MTLRenderCommandEncoder, MTLRenderPassDescriptor, MTLRenderPipelineState, MTLSamplerAddressMode,
    MTLSamplerDescriptor, MTLSamplerMinMagFilter, MTLSamplerMipFilter, MTLSamplerState, MTLScissorRect, MTLStoreAction,
};
use std::ptr::NonNull;

fn mtl_wrap(wrap: Wrap) -> MTLSamplerAddressMode {
    match wrap {
        Wrap::Clamp => MTLSamplerAddressMode::ClampToEdge,
        Wrap::Repeat => MTLSamplerAddressMode::Repeat,
        Wrap::Mirror => MTLSamplerAddressMode::MirrorRepeat,
    }
}

/// The part of `a` within `b`.
fn intersect(a: IRect, b: IRect) -> IRect {
    let (x0, y0) = (a.x.max(b.x), a.y.max(b.y));
    let (x1, y1) = ((a.x + a.width).min(b.x + b.width), (a.y + a.height).min(b.y + b.height));
    IRect::new(x0, y0, (x1 - x0).max(0), (y1 - y0).max(0))
}

impl MetalDevice {
    pub(super) fn start_pass(&mut self, target: TextureId, clear: Option<ClearColor>, area: Option<IRect>) {
        self.end_blit();
        let label = self.pass_label(target);
        self.time_next(|| label);
        let bounds = target.bounds();
        let pass_area = intersect(area.unwrap_or(bounds), bounds);
        let whole = pass_area == bounds;
        let Some(t) = self.textures.get_mut(target) else {
            panic!("Device::begin_pass: {target:?} isn't the device's (or is gone)");
        };
        let discarded = std::mem::take(&mut t.discarded);
        let texture = t.texture.clone();
        // A clear and "don't care" are all of the target's on Metal: with an area, what is outside it is loaded (and
        // kept) instead, and the area cleared by a draw.
        let load = match clear {
            Some(_) if whole => MTLLoadAction::Clear,
            None if discarded && whole => MTLLoadAction::DontCare,
            _ => MTLLoadAction::Load,
        };
        let descriptor = MTLRenderPassDescriptor::renderPassDescriptor();
        // SAFETY: attachment 0, within Metal's 8.
        let attachment = unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) };
        attachment.setTexture(Some(&texture));
        attachment.setLoadAction(load);
        attachment.setStoreAction(MTLStoreAction::Store);
        if let Some(c) = clear {
            attachment.setClearColor(MTLClearColor {
                red: c.r as f64,
                green: c.g as f64,
                blue: c.b as f64,
                alpha: c.a as f64,
            });
        }
        let encoder = self.commands().renderCommandEncoderWithDescriptor(&descriptor).expect("Metal: a render encoder");
        self.pass = Some(encoder);
        self.recorded = true;
        self.target = Some(target);
        self.bound = [(None, Sampler::default()); MAX_TEXTURES];
        self.pass_area = pass_area;
        if let Some(c) = clear
            && !whole
            && !pass_area.is_empty()
            && let Some(pipeline) = self.clear_pipeline(target.format())
        {
            let encoder = self.encoder();
            encoder.setRenderPipelineState(&pipeline);
            encoder.setScissorRect(MTLScissorRect {
                x: pass_area.x as usize,
                y: pass_area.y as usize,
                width: pass_area.width as usize,
                height: pass_area.height as usize,
            });
            let color = [c.r, c.g, c.b, c.a];
            // SAFETY: 16 bytes of the color, read during the call; three vertices of one triangle.
            unsafe {
                encoder.setFragmentBytes_length_atIndex(NonNull::from(&color).cast(), size_of_val(&color), 0);
                encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Triangle, 0, 3);
            }
        }
        self.set_pass_scissor(pass_area);
    }

    /// The device's pipeline clearing part of a `format` target (made once).
    fn clear_pipeline(&mut self, format: Format) -> Option<Retained<ProtocolObject<dyn MTLRenderPipelineState>>> {
        if let Some((_, pipeline)) = self.clears.iter().find(|(f, _)| *f == format) {
            return Some(pipeline.clone());
        }
        let pipeline = self.maker.clear_pipeline(format)?;
        self.clears.push((format, pipeline.clone()));
        Some(pipeline)
    }

    pub(super) fn set_pass_scissor(&mut self, rect: IRect) {
        let target = self.target.expect("a pass begun");
        let r = intersect(intersect(rect, self.pass_area), target.bounds());
        self.scissor_empty = r.is_empty();
        if !self.scissor_empty {
            self.encoder().setScissorRect(MTLScissorRect {
                x: r.x as usize,
                y: r.y as usize,
                width: r.width as usize,
                height: r.height as usize,
            });
        }
    }

    pub(super) fn push_uniforms(&mut self, data: &[u8]) {
        let data = &data[..data.len().min(MAX_UNIFORMS)];
        if data.is_empty() {
            return;
        }
        let bytes = NonNull::from(data).cast();
        let encoder = self.encoder();
        // SAFETY: `data.len()` bytes from `bytes`, read (copied) during the calls; buffer 0 is the uniforms' in every
        // shader.
        unsafe {
            encoder.setVertexBytes_length_atIndex(bytes, data.len(), 0);
            encoder.setFragmentBytes_length_atIndex(bytes, data.len(), 0);
        }
    }

    /// The sampler state of a sampler (made once).
    fn sampler_state(&mut self, s: Sampler) -> Retained<ProtocolObject<dyn MTLSamplerState>> {
        if let Some((_, state)) = self.samplers.iter().find(|(desc, _)| *desc == s) {
            return state.clone();
        }
        let d = MTLSamplerDescriptor::new();
        let filter =
            if s.filter == Filter::Linear { MTLSamplerMinMagFilter::Linear } else { MTLSamplerMinMagFilter::Nearest };
        d.setMinFilter(filter);
        d.setMagFilter(filter);
        // (Mipmaps: the nearest one, as Skia's FilterQuality::Medium and the other backends take.)
        d.setMipFilter(if s.mipmaps { MTLSamplerMipFilter::Nearest } else { MTLSamplerMipFilter::NotMipmapped });
        d.setSAddressMode(mtl_wrap(s.wrap_u));
        d.setTAddressMode(mtl_wrap(s.wrap_v));
        let state = self.device.newSamplerStateWithDescriptor(&d).expect("Metal: a sampler state");
        self.samplers.push((s, state.clone()));
        state
    }

    pub(super) fn bind_texture(&mut self, slot: usize, id: TextureId, sampler: Sampler) {
        assert!(slot < MAX_TEXTURES, "set_texture: slot {slot} past {MAX_TEXTURES}");
        if self.bound[slot] == (Some(id), sampler) {
            return;
        }
        let Some(texture) = self.textures.texture(id) else { return };
        self.bound[slot] = (Some(id), sampler);
        let state = self.sampler_state(sampler);
        let encoder = self.encoder();
        // SAFETY: the slot is within the textures and samplers every pipeline has (MAX_TEXTURES, Metal's 31).
        unsafe {
            encoder.setFragmentTexture_atIndex(Some(&texture), slot);
            encoder.setFragmentSamplerState_atIndex(Some(&state), slot);
        }
    }

    pub(super) fn draw_instances(&mut self, instances: Option<BufferRef>, vertex_count: u32, instance_count: u32) {
        if instance_count == 0 || self.scissor_empty {
            return;
        }
        let buffer = instances.map(|b| (self.chunk(b).retain(), b.offset()));
        let encoder = self.encoder();
        // SAFETY: the instances are in this frame's memory, at their offset (buffer 1: the pipelines' instances); the
        // pipeline's vertex shader makes each instance's vertices from the vertex index.
        unsafe {
            if let Some((buffer, offset)) = &buffer {
                encoder.setVertexBuffer_offset_atIndex(Some(buffer), *offset as usize, 1);
            }
            encoder.drawPrimitives_vertexStart_vertexCount_instanceCount(
                MTLPrimitiveType::Triangle,
                0,
                vertex_count as usize,
                instance_count as usize,
            );
        }
    }

    pub(super) fn finish_pass(&mut self) {
        if let Some(pass) = self.pass.take() {
            pass.endEncoding();
        }
        self.target = None;
        self.time_done();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn areas_are_cut_to_their_targets() {
        let bounds = IRect::new(0, 0, 48, 32);
        assert_eq!(intersect(IRect::new(-10, 0, 34, 100), bounds), IRect::new(0, 0, 24, 32));
        assert_eq!(intersect(IRect::new(40, 30, 20, 20), bounds), IRect::new(40, 30, 8, 2));
        assert!(intersect(IRect::new(50, 0, 4, 4), bounds).is_empty());
    }
}
