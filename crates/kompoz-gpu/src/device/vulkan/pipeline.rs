//! Pipelines (made on any thread, through the driver's pipeline cache, kept in its file between runs), the render
//! passes they and the passes are made for, and samplers.

use super::VulkanDevice;
use super::texture::vk_format;
use crate::device::{
    BlendFactor, BlendOp, Filter, Pipeline, PipelineDesc, PipelineFactory, PipelineId, RetiredQueue, Sampler,
    VertexFormat, Wrap, log, read_pipeline_cache, write_pipeline_cache,
};
use ash::vk;
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};

fn vk_factor(f: BlendFactor) -> vk::BlendFactor {
    match f {
        BlendFactor::Zero => vk::BlendFactor::ZERO,
        BlendFactor::One => vk::BlendFactor::ONE,
        BlendFactor::SrcAlpha => vk::BlendFactor::SRC_ALPHA,
        BlendFactor::OneMinusSrcAlpha => vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
        BlendFactor::DstAlpha => vk::BlendFactor::DST_ALPHA,
        BlendFactor::OneMinusDstAlpha => vk::BlendFactor::ONE_MINUS_DST_ALPHA,
        BlendFactor::SrcColor => vk::BlendFactor::SRC_COLOR,
        BlendFactor::OneMinusSrcColor => vk::BlendFactor::ONE_MINUS_SRC_COLOR,
        BlendFactor::DstColor => vk::BlendFactor::DST_COLOR,
        BlendFactor::OneMinusDstColor => vk::BlendFactor::ONE_MINUS_DST_COLOR,
    }
}

fn vk_op(op: BlendOp) -> vk::BlendOp {
    match op {
        BlendOp::Add => vk::BlendOp::ADD,
        BlendOp::Subtract => vk::BlendOp::SUBTRACT,
        BlendOp::ReverseSubtract => vk::BlendOp::REVERSE_SUBTRACT,
        BlendOp::Min => vk::BlendOp::MIN,
        BlendOp::Max => vk::BlendOp::MAX,
    }
}

fn vk_vertex_format(f: VertexFormat) -> vk::Format {
    match f {
        VertexFormat::Float => vk::Format::R32_SFLOAT,
        VertexFormat::Float2 => vk::Format::R32G32_SFLOAT,
        VertexFormat::Float3 => vk::Format::R32G32B32_SFLOAT,
        VertexFormat::Float4 => vk::Format::R32G32B32A32_SFLOAT,
        VertexFormat::UInt4 => vk::Format::R32G32B32A32_UINT,
    }
}

fn vk_wrap(w: Wrap) -> vk::SamplerAddressMode {
    match w {
        Wrap::Clamp => vk::SamplerAddressMode::CLAMP_TO_EDGE,
        Wrap::Repeat => vk::SamplerAddressMode::REPEAT,
        Wrap::Mirror => vk::SamplerAddressMode::MIRRORED_REPEAT,
    }
}

/// What makes pipelines, shared with other threads ([`PipelineFactory`]): the device's function table, the one
/// pipeline layout, the pipeline cache (the driver synchronizes it), and the render passes pipelines and passes are
/// made for. Made unusable when the device goes ([`PipelineMaker::shut`]).
pub(super) struct PipelineMaker {
    device: ash::Device,
    layout: vk::PipelineLayout,
    pub cache: vk::PipelineCache,
    render_passes: Mutex<HashMap<(vk::Format, vk::AttachmentLoadOp), vk::RenderPass>>,
    /// Pipelines made and not destroyed yet (destroyed with the device if their [`Pipeline`]s outlive it).
    live: Mutex<HashSet<u64>>,
    /// Pipelines were made since the cache was last kept (at a frame's end).
    pub made: AtomicBool,
    /// False once the device is gone: nothing is made after.
    alive: RwLock<bool>,
    retired: Arc<RetiredQueue>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl PipelineMaker {
    pub fn new(
        device: ash::Device,
        layout: vk::PipelineLayout,
        cache: vk::PipelineCache,
        retired: Arc<RetiredQueue>,
    ) -> PipelineMaker {
        PipelineMaker {
            device,
            layout,
            cache,
            render_passes: Mutex::default(),
            live: Mutex::default(),
            made: AtomicBool::new(false),
            alive: RwLock::new(true),
            retired,
        }
    }

    /// The render pass for targets of `format` starting as `load` says (cleared, loaded, or neither), made once.
    /// Initial and final layouts COLOR_ATTACHMENT_OPTIMAL, one sample.
    pub fn render_pass(&self, format: vk::Format, load: vk::AttachmentLoadOp) -> vk::RenderPass {
        let mut passes = lock(&self.render_passes);
        if let Some(&pass) = passes.get(&(format, load)) {
            return pass;
        }
        let attachment = vk::AttachmentDescription::default()
            .format(format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(load)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
        let reference =
            [vk::AttachmentReference::default().attachment(0).layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)];
        let subpass = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&reference);
        let attachments = [attachment];
        let subpasses = [subpass];
        let info = vk::RenderPassCreateInfo::default().attachments(&attachments).subpasses(&subpasses);
        // SAFETY: a valid render pass info, for the device (alive: its callers hold it, or the alive lock).
        let pass = unsafe { self.device.create_render_pass(&info, None) }.unwrap_or_default();
        passes.insert((format, load), pass);
        pass
    }

    /// Takes a pipeline let go of out of those to destroy with the device; whether it was one of them.
    pub fn forget(&self, pipeline: vk::Pipeline) -> bool {
        use ash::vk::Handle;
        lock(&self.live).remove(&pipeline.as_raw())
    }

    /// Nothing made from now on; the render passes and the pipelines still there, to destroy with the device.
    pub fn shut(&self) -> (Vec<vk::RenderPass>, Vec<vk::Pipeline>) {
        use ash::vk::Handle;
        *self.alive.write().unwrap_or_else(|e| e.into_inner()) = false;
        let passes = lock(&self.render_passes).drain().map(|(_, pass)| pass).collect();
        let pipelines = lock(&self.live).drain().map(vk::Pipeline::from_raw).collect();
        (passes, pipelines)
    }

    fn make(&self, desc: &PipelineDesc<'_>) -> Option<Pipeline> {
        use ash::vk::Handle;
        let alive = self.alive.read().unwrap_or_else(|e| e.into_inner());
        if !*alive {
            return None;
        }
        let module = |code: &[u8]| -> Option<vk::ShaderModule> {
            if code.is_empty() || !code.len().is_multiple_of(4) {
                return None;
            }
            let words: Vec<u32> = code.as_chunks::<4>().0.iter().map(|w| u32::from_le_bytes(*w)).collect();
            let info = vk::ShaderModuleCreateInfo::default().code(&words);
            // SAFETY: SPIR-V words for the device (the shaders' generated code).
            unsafe { self.device.create_shader_module(&info, None) }.ok()
        };
        let vertex = module(desc.vertex.spirv)?;
        let Some(fragment) = module(desc.fragment.spirv) else {
            // SAFETY: nothing uses it.
            unsafe { self.device.destroy_shader_module(vertex, None) };
            return None;
        };
        let (vertex_entry, fragment_entry) = (
            CString::new(desc.vertex.entry).unwrap_or_default(),
            CString::new(desc.fragment.entry).unwrap_or_default(),
        );
        let entries: Vec<vk::SpecializationMapEntry> = (0..desc.constants.len() as u32)
            .map(|i| vk::SpecializationMapEntry::default().constant_id(i).offset(i * 4).size(4))
            .collect();
        let constants: Vec<u8> = desc.constants.iter().flat_map(|c| c.to_ne_bytes()).collect();
        let specialization = vk::SpecializationInfo::default().map_entries(&entries).data(&constants);
        let mut fragment_stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(fragment)
            .name(&fragment_entry);
        if !desc.constants.is_empty() {
            fragment_stage = fragment_stage.specialization_info(&specialization);
        }
        let stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vertex)
                .name(&vertex_entry),
            fragment_stage,
        ];
        let bindings = [vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(desc.instance_stride)
            .input_rate(vk::VertexInputRate::INSTANCE)];
        let attributes: Vec<vk::VertexInputAttributeDescription> = desc
            .instance_attributes
            .iter()
            .map(|a| {
                vk::VertexInputAttributeDescription::default()
                    .location(a.location)
                    .binding(0)
                    .format(vk_vertex_format(a.format))
                    .offset(a.offset)
            })
            .collect();
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(if desc.instance_stride > 0 { &bindings } else { &[] })
            .vertex_attribute_descriptions(&attributes);
        let input_assembly =
            vk::PipelineInputAssemblyStateCreateInfo::default().topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        let viewport = vk::PipelineViewportStateCreateInfo::default().viewport_count(1).scissor_count(1);
        let rasterization = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::NONE)
            .line_width(1.0);
        let multisample =
            vk::PipelineMultisampleStateCreateInfo::default().rasterization_samples(vk::SampleCountFlags::TYPE_1);
        let blend = desc.blend;
        let attachment = [vk::PipelineColorBlendAttachmentState::default()
            .blend_enable(blend.enabled)
            .src_color_blend_factor(vk_factor(blend.src_color))
            .dst_color_blend_factor(vk_factor(blend.dst_color))
            .color_blend_op(vk_op(blend.color_op))
            .src_alpha_blend_factor(vk_factor(blend.src_alpha))
            .dst_alpha_blend_factor(vk_factor(blend.dst_alpha))
            .alpha_blend_op(vk_op(blend.alpha_op))
            .color_write_mask(vk::ColorComponentFlags::RGBA)];
        let color_blend = vk::PipelineColorBlendStateCreateInfo::default().attachments(&attachment);
        let dynamics = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamics);
        // (Made for the pass that loads; compatible with the others too.)
        let render_pass = self.render_pass(vk_format(desc.target), vk::AttachmentLoadOp::LOAD);
        let info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport)
            .rasterization_state(&rasterization)
            .multisample_state(&multisample)
            .color_blend_state(&color_blend)
            .dynamic_state(&dynamic)
            .layout(self.layout)
            .render_pass(render_pass);
        // SAFETY: the info and everything it points to live through the call; the modules, layout, cache and render
        // pass are the device's, which is alive (the read lock is held).
        let made = unsafe {
            let made = self.device.create_graphics_pipelines(self.cache, &[info], None);
            self.device.destroy_shader_module(vertex, None);
            self.device.destroy_shader_module(fragment, None);
            made
        };
        match made {
            Ok(pipelines) if pipelines[0] != vk::Pipeline::null() => {
                self.made.store(true, Ordering::Relaxed);
                lock(&self.live).insert(pipelines[0].as_raw());
                Some(Pipeline::new(PipelineId::new(pipelines[0].as_raw()), self.retired.clone()))
            }
            Ok(_) => None,
            Err((_, e)) => {
                let constants: String = desc.constants.iter().map(|c| format!(" {c}")).collect();
                let constants =
                    if constants.is_empty() { String::new() } else { format!(", its constants{constants}") };
                log(&format!("Vulkan: a pipeline wasn't made ({}){constants}", e.as_raw()));
                None
            }
        }
    }
}

impl PipelineFactory for PipelineMaker {
    fn create_pipeline(&self, desc: &PipelineDesc<'_>) -> Option<Pipeline> {
        self.make(desc)
    }
}

/// The header a driver's pipeline cache data begins with: its size, version one, the vendor's and device's IDs,
/// and the driver's cache UUID. Drivers check it too, though not every one as carefully.
pub(super) fn cache_is_ours(data: &[u8], properties: &vk::PhysicalDeviceProperties) -> bool {
    let header = 16 + vk::UUID_SIZE;
    if data.len() < header {
        return false;
    }
    let u32_at = |at: usize| u32::from_ne_bytes(data[at..at + 4].try_into().unwrap());
    u32_at(0) as usize >= header
        && u32_at(4) == vk::PipelineCacheHeaderVersion::ONE.as_raw() as u32
        && u32_at(8) == properties.vendor_id
        && u32_at(12) == properties.device_id
        && data[16..header] == properties.pipeline_cache_uuid
}

impl VulkanDevice {
    /// The pipeline cache's data: what the driver has.
    fn cache_data_size(&self) -> Option<usize> {
        let mut size = 0usize;
        // SAFETY: a query of the cache's size, without data.
        let result = unsafe {
            (self.device.fp_v1_0().get_pipeline_cache_data)(
                self.device.handle(),
                self.maker.cache,
                &mut size,
                std::ptr::null_mut(),
            )
        };
        (result == vk::Result::SUCCESS).then_some(size)
    }

    /// The pipeline cache made from its file (where the file is this driver's) and how much the driver took of it.
    pub(super) fn read_cache_size(&mut self) {
        self.cache_size = self.cache_data_size().unwrap_or(0);
    }

    /// The pipeline cache into its file, if it has more than it had. (Told by its size: what it has more makes it
    /// bigger, while the same cache may be written in another order each time, as Mesa's driver for Intel writes it.)
    pub(super) fn keep_pipeline_cache(&mut self) {
        self.maker.made.store(false, Ordering::Relaxed);
        let Some(file) = self.cache_file.clone() else { return };
        if self.maker.cache == vk::PipelineCache::null()
            || self.cache_data_size().is_none_or(|size| size == self.cache_size)
        {
            return;
        }
        // SAFETY: the cache is the device's.
        let Ok(data) = (unsafe { self.device.get_pipeline_cache_data(self.maker.cache) }) else { return };
        write_pipeline_cache(&file, &data);
        self.cache_size = data.len();
    }

    pub(super) fn sampler(&mut self, s: Sampler) -> vk::Sampler {
        if let Some((_, sampler)) = self.samplers.iter().find(|(desc, _)| *desc == s) {
            return *sampler;
        }
        let filter = if s.filter == Filter::Linear { vk::Filter::LINEAR } else { vk::Filter::NEAREST };
        let info = vk::SamplerCreateInfo::default()
            .mag_filter(filter)
            .min_filter(filter)
            // (Skia's for FilterQuality::Medium.)
            .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
            .address_mode_u(vk_wrap(s.wrap_u))
            .address_mode_v(vk_wrap(s.wrap_v))
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .max_lod(if s.mipmaps { vk::LOD_CLAMP_NONE } else { 0.0 });
        // SAFETY: a valid sampler info.
        let sampler = unsafe { self.device.create_sampler(&info, None) }.unwrap_or_default();
        self.samplers.push((s, sampler));
        sampler
    }

    /// The pipeline cache, from its file where that is this driver's.
    pub(super) fn load_pipeline_cache(
        device: &ash::Device,
        file: Option<&std::path::Path>,
        properties: &vk::PhysicalDeviceProperties,
    ) -> Result<vk::PipelineCache, vk::Result> {
        let mut kept = file.map(read_pipeline_cache).unwrap_or_default();
        if !cache_is_ours(&kept, properties) {
            kept.clear();
        }
        let info = vk::PipelineCacheCreateInfo::default().initial_data(&kept);
        // SAFETY: a valid cache info whose data the driver checks (and is this driver's, as far as its header says).
        unsafe { device.create_pipeline_cache(&info, None) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caches_are_ours_by_their_header() {
        let mut properties =
            vk::PhysicalDeviceProperties { vendor_id: 0x8086, device_id: 0xa7a0, ..Default::default() };
        properties.pipeline_cache_uuid = [7; vk::UUID_SIZE];
        let mut data = Vec::new();
        data.extend_from_slice(&32u32.to_ne_bytes());
        data.extend_from_slice(&1u32.to_ne_bytes());
        data.extend_from_slice(&0x8086u32.to_ne_bytes());
        data.extend_from_slice(&0xa7a0u32.to_ne_bytes());
        data.extend_from_slice(&[7; vk::UUID_SIZE]);
        data.extend_from_slice(b"the driver's own");
        assert!(cache_is_ours(&data, &properties));
        assert!(!cache_is_ours(&data[..31], &properties));
        // Its header's size too small, another version, vendor, device or driver: not ours.
        for at in [0, 4, 8, 12, 16, 31] {
            let mut other = data.clone();
            other[at] ^= if at == 0 { 0x3f } else { 1 };
            assert!(!cache_is_ours(&other, &properties), "byte {at}");
        }
    }
}
