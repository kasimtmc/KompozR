//! Render passes as they are recorded: the target made an attachment and the textures read made readable before,
//! a render pass (cleared, loaded, or neither) over the pass's area with its framebuffer, then pipelines, scissors,
//! push constants, descriptor sets and instanced draws.

use super::VulkanDevice;
use super::texture::{transition, vk_format};
use crate::device::{
    BufferRef, ClearColor, IRect, MAX_TEXTURES, MAX_UNIFORMS, PipelineId, Sampler, TextureId, TextureUsage,
};
use ash::vk;
use ash::vk::Handle;

impl VulkanDevice {
    pub(super) fn start_pass(
        &mut self,
        target: TextureId,
        clear: Option<ClearColor>,
        reads: &[TextureId],
        area: Option<IRect>,
    ) {
        self.make_readable(reads, target);
        let label = self.pass_label_of(target);
        let (device, cmd) = (&self.device, self.cmd);
        let t = self.textures.get_mut(target).expect("begin_pass: a target of this device's (and not let go of)");
        assert!(t.usage.contains(TextureUsage::RENDER_TARGET), "begin_pass: {target:?} isn't a render target");
        // SAFETY: the frame's command buffer records outside passes.
        unsafe { transition(device, cmd, t, vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL) };
        let load = match (clear, t.discarded) {
            (Some(_), _) => vk::AttachmentLoadOp::CLEAR,
            (None, true) => vk::AttachmentLoadOp::DONT_CARE,
            (None, false) => vk::AttachmentLoadOp::LOAD,
        };
        t.discarded = false;
        let pass = self.maker.render_pass(vk_format(target.format()), load);
        let framebuffer = match t.framebuffers.iter().find(|(p, _)| *p == pass) {
            Some((_, framebuffer)) => *framebuffer,
            None => {
                let views = [t.view];
                let info = vk::FramebufferCreateInfo::default()
                    .render_pass(pass)
                    .attachments(&views)
                    .width(target.width() as u32)
                    .height(target.height() as u32)
                    .layers(1);
                // SAFETY: the view and the pass are the device's, the pass of the view's format.
                let framebuffer = unsafe { device.create_framebuffer(&info, None) }.unwrap_or_default();
                t.framebuffers.push((pass, framebuffer));
                framebuffer
            }
        };
        let (w, h) = (target.width(), target.height());
        let (x0, y0, x1, y1) = match area {
            Some(a) => {
                let (x0, y0) = (a.x.clamp(0, w), a.y.clamp(0, h));
                (x0, y0, (a.x.saturating_add(a.width)).clamp(x0, w), (a.y.saturating_add(a.height)).clamp(y0, h))
            }
            None => (0, 0, w, h),
        };
        self.pass_area = IRect::new(x0, y0, x1 - x0, y1 - y0);
        let clear_values = [vk::ClearValue {
            color: vk::ClearColorValue { float32: clear.map_or([0.0; 4], |c| [c.r, c.g, c.b, c.a]) },
        }];
        let info = vk::RenderPassBeginInfo::default()
            .render_pass(pass)
            .framebuffer(framebuffer)
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: x0, y: y0 },
                extent: vk::Extent2D { width: (x1 - x0) as u32, height: (y1 - y0) as u32 },
            })
            .clear_values(if clear.is_some() { &clear_values } else { &[] });
        self.recorded = true;
        self.mark(vk::PipelineStageFlags::TOP_OF_PIPE, label);
        let viewport =
            vk::Viewport { x: 0.0, y: 0.0, width: w as f32, height: h as f32, min_depth: 0.0, max_depth: 1.0 };
        // SAFETY: the frame's command buffer records outside passes; the framebuffer is the target's for this pass.
        unsafe {
            self.device.cmd_begin_render_pass(self.cmd, &info, vk::SubpassContents::INLINE);
            self.device.cmd_set_viewport(self.cmd, 0, &[viewport]);
        }
        self.target = Some(target);
        self.set_pass_scissor(self.pass_area);
        self.textures_dirty = true;
    }

    pub(super) fn use_pipeline(&mut self, pipeline: PipelineId) {
        let pipeline = vk::Pipeline::from_raw(pipeline.raw());
        if pipeline == self.bound_pipeline {
            return;
        }
        // SAFETY: the pipeline is the device's (alive: its destruction waits for the frame's fence), made for a pass
        // compatible with this one.
        unsafe { self.device.cmd_bind_pipeline(self.cmd, vk::PipelineBindPoint::GRAPHICS, pipeline) };
        self.bound_pipeline = pipeline;
    }

    /// The scissor, within the pass's area.
    pub(super) fn set_pass_scissor(&mut self, r: IRect) {
        let a = self.pass_area;
        let (x0, y0) = (a.x.max(r.x), a.y.max(r.y));
        let (x1, y1) =
            ((r.x.saturating_add(r.width)).min(a.x + a.width), (r.y.saturating_add(r.height)).min(a.y + a.height));
        let rect = vk::Rect2D {
            offset: vk::Offset2D { x: x0, y: y0 },
            extent: vk::Extent2D { width: (x1 - x0).max(0) as u32, height: (y1 - y0).max(0) as u32 },
        };
        // SAFETY: the frame's command buffer records, in a pass.
        unsafe { self.device.cmd_set_scissor(self.cmd, 0, &[rect]) };
    }

    pub(super) fn push_uniforms(&mut self, data: &[u8]) {
        let data = &data[..data.len().min(MAX_UNIFORMS) & !3];
        if data.is_empty() {
            return;
        }
        // SAFETY: the frame's command buffer records; the range is within the layout's push constants (both stages).
        unsafe {
            self.device.cmd_push_constants(
                self.cmd,
                self.pipeline_layout,
                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                0,
                data,
            )
        };
    }

    pub(super) fn bind_texture(&mut self, slot: usize, texture: TextureId, sampler: Sampler) {
        assert!(slot < MAX_TEXTURES, "set_texture: slot {slot} of {MAX_TEXTURES}");
        if self.bound[slot] == (Some(texture), sampler) {
            return;
        }
        // (A texture that isn't sampled can't be in a descriptor set.)
        assert!(
            self.textures.get(texture).is_none_or(|t| t.usage.contains(TextureUsage::SAMPLED)),
            "set_texture: {texture:?} isn't sampled"
        );
        self.bound[slot] = (Some(texture), sampler);
        self.textures_dirty = true;
    }

    pub(super) fn draw_instances(&mut self, instances: Option<BufferRef>, vertex_count: u32, instance_count: u32) {
        if instance_count == 0 {
            return;
        }
        if self.textures_dirty {
            self.bind_textures();
        }
        // SAFETY: the frame's command buffer records, in a pass, with a pipeline bound; the buffer is the frame's
        // (alive until its fence).
        unsafe {
            if let Some(instances) = instances {
                self.device.cmd_bind_vertex_buffers(
                    self.cmd,
                    0,
                    &[vk::Buffer::from_raw(instances.buffer())],
                    &[instances.offset()],
                );
            }
            self.device.cmd_draw(self.cmd, vertex_count, instance_count, 0, 0);
        }
    }

    pub(super) fn finish_pass(&mut self) {
        // SAFETY: the frame's command buffer records, in a pass.
        unsafe { self.device.cmd_end_render_pass(self.cmd) };
        self.mark(vk::PipelineStageFlags::BOTTOM_OF_PIPE, String::new());
        self.target = None;
    }
}
