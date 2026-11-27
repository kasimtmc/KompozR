//! Frames: their command buffers (committed in order on the one queue), the shared buffers their memory comes from,
//! what was let go of while the GPU may still use it, the drawable, and the GPU's times of timed frames.

use super::MetalDevice;
use crate::device::{BufferRef, GpuFrameTime, GpuPassTime, Retired, TextureId, log};
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_core_foundation::CGSize;
use objc2_metal::{
    MTLBuffer, MTLCommandBuffer, MTLCommandBufferDescriptor, MTLCommandBufferErrorOption, MTLCommandBufferStatus,
    MTLCommandEncoder, MTLCommandQueue, MTLDevice, MTLRenderPipelineState, MTLResourceOptions, MTLTexture,
};
use objc2_quartz_core::CAMetalDrawable;
use std::ptr::NonNull;
use std::sync::atomic::Ordering;

/// Frames the CPU records while the GPU draws those before.
pub(super) const FRAMES_IN_FLIGHT: usize = 2;
/// A frame's memory comes in pieces this big (or as big as asked).
const CHUNK_SIZE: usize = 4 << 20;

/// A shared buffer of a frame's memory, and how much of it is handed out.
pub(super) struct Chunk {
    pub buffer: Retained<ProtocolObject<dyn MTLBuffer>>,
    pub used: usize,
}

/// What a frame in flight has.
#[derive(Default)]
pub(super) struct Frame {
    /// Its last command buffer committed: once it is done, so is all of the frame (one queue runs them in order).
    pub submitted: Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>>,
    pub chunks: Vec<Chunk>,
    /// What was let go of in it, kept until the GPU is done with it.
    pub textures: Vec<Retained<ProtocolObject<dyn MTLTexture>>>,
    pub pipelines: Vec<Retained<ProtocolObject<dyn MTLRenderPipelineState>>>,
    /// Timed: its passes', copies' and mipmap chains' command buffers, each with its label.
    pub timed: Vec<(String, Retained<ProtocolObject<dyn MTLCommandBuffer>>)>,
}

impl MetalDevice {
    pub(super) fn start_frame(&mut self) {
        self.frame_index = (self.frame_index + 1) % FRAMES_IN_FLIGHT;
        let frame = &mut self.frames[self.frame_index];
        if let Some(submitted) = frame.submitted.take() {
            submitted.waitUntilCompleted();
        }
        frame.textures.clear();
        frame.pipelines.clear();
        let timed = std::mem::take(&mut frame.timed);
        for chunk in &mut frame.chunks {
            chunk.used = 0;
        }
        // Its times: each pass's command buffer from when the GPU started it to when it ended, the frame theirs
        // together (not the waits between them).
        let mut time = GpuFrameTime::default();
        for (label, commands) in timed {
            let ms = (commands.GPUEndTime() - commands.GPUStartTime()).max(0.0) * 1000.0;
            time.passes.push(GpuPassTime { label, ms });
            time.ms += ms;
        }
        if !time.passes.is_empty() {
            self.timed.push(time);
        }
        self.drawable = None;
        self.window = None;
        self.in_frame = true;
        self.start_commands();
    }

    /// A command buffer to record into, its failure counted and told once the GPU is done with it.
    pub(super) fn start_commands(&mut self) {
        let commands = if self.validating {
            // (With each encoder's status in its error: which pass failed.)
            let descriptor = MTLCommandBufferDescriptor::new();
            descriptor.setErrorOptions(MTLCommandBufferErrorOption::EncoderExecutionStatus);
            self.queue.commandBufferWithDescriptor(&descriptor)
        } else {
            self.queue.commandBuffer()
        };
        let commands = commands.expect("Metal: a command buffer");
        let errors = self.errors.clone();
        let handler = RcBlock::new(move |done: NonNull<ProtocolObject<dyn MTLCommandBuffer>>| {
            // SAFETY: Metal calls the handler with the command buffer it was added to, alive for the call.
            let done = unsafe { done.as_ref() };
            if done.status() == MTLCommandBufferStatus::Error {
                log(&format!("Metal: a command buffer failed: {}", super::describe(done.error().as_deref())));
                errors.fetch_add(1, Ordering::Relaxed);
            }
        });
        // SAFETY: the block takes the command buffer as Metal's handlers do; Metal copies it (it outlives `handler`).
        unsafe { commands.addCompletedHandler(RcBlock::as_ptr(&handler)) };
        self.commands = Some(commands);
        self.recorded = false;
        self.label = None;
    }

    /// The command buffer recording.
    pub(super) fn commands(&self) -> &ProtocolObject<dyn MTLCommandBuffer> {
        self.commands.as_deref().expect("a frame begun")
    }

    pub(super) fn end_blit(&mut self) {
        if let Some(blit) = self.blit.take() {
            blit.endEncoding();
        }
    }

    /// Commits what is recorded (timed, with its label) and starts another command buffer.
    pub(super) fn commit_commands(&mut self) {
        self.end_blit();
        if !self.recorded {
            self.label = None;
            return;
        }
        let commands = self.commands.take().expect("a frame begun");
        commands.commit();
        if let Some(label) = self.label.take() {
            self.frames[self.frame_index].timed.push((label, commands));
        }
        self.start_commands();
    }

    /// Timed: what comes next in a command buffer of its own named `label` (a pass, copy or mipmap chain), what was
    /// recorded before committed.
    pub(super) fn time_next(&mut self, label: impl FnOnce() -> String) {
        if self.timing {
            self.commit_commands();
            self.label = Some(label());
        }
    }

    /// Timed: what was named by [`time_next`](Self::time_next) committed, the rest of the frame in another.
    pub(super) fn time_done(&mut self) {
        if self.timing {
            self.commit_commands();
        }
    }

    /// The drawable's texture for this frame, the layer at the drawable's size; none without a layer, or minimized.
    pub(super) fn acquire(&mut self) -> Option<TextureId> {
        let layer = self.layer.clone()?;
        if let Some(window) = self.window {
            return Some(window);
        }
        let (w, h) = self.drawable_size.as_ref().map_or((0, 0), |size| size());
        if w <= 0 || h <= 0 {
            return None; // (minimized)
        }
        if self.layer_size != (w, h) {
            layer.setDrawableSize(CGSize { width: w as f64, height: h as f64 });
            self.layer_size = (w, h);
        }
        let drawable = layer.nextDrawable()?;
        let texture = drawable.texture();
        let id = self.textures.insert(texture, crate::device::Format::Bgra8);
        self.drawable = Some(drawable);
        self.window = Some(id);
        Some(id)
    }

    /// `data` in this frame's memory, at a multiple of `alignment` (16 at least).
    pub(super) fn write_data(&mut self, data: &[u8], alignment: usize) -> Option<BufferRef> {
        self.write_with(data.len(), alignment, |memory| memory.copy_from_slice(data))
    }

    /// `len` bytes of this frame's memory at a multiple of `alignment` (16 at least), written by `write`.
    pub(super) fn write_with(
        &mut self,
        len: usize,
        alignment: usize,
        write: impl FnOnce(&mut [u8]),
    ) -> Option<BufferRef> {
        let alignment = alignment.max(16);
        let frame = &mut self.frames[self.frame_index];
        let found = frame.chunks.iter().position(|c| c.used.next_multiple_of(alignment) + len <= c.buffer.length());
        let index = match found {
            Some(index) => index,
            None => {
                let options = MTLResourceOptions::StorageModeShared | MTLResourceOptions::CPUCacheModeWriteCombined;
                let buffer = self.device.newBufferWithLength_options(CHUNK_SIZE.max(len), options)?;
                frame.chunks.push(Chunk { buffer, used: 0 });
                frame.chunks.len() - 1
            }
        };
        let chunk = &mut frame.chunks[index];
        let start = chunk.used.next_multiple_of(alignment);
        // SAFETY: the shared buffer's memory is the CPU's from `contents` for its length, which holds `start + len`;
        // the GPU reads none of it before this frame's work is committed (the frame that used it before is done:
        // waited for at the frame's start), and nothing else of it is handed out twice.
        let memory =
            unsafe { std::slice::from_raw_parts_mut(chunk.buffer.contents().as_ptr().cast::<u8>().add(start), len) };
        write(memory);
        chunk.used = start + len;
        Some(BufferRef::new(index as u64, start as u64))
    }

    /// The chunk a [`BufferRef`] of this frame's is in.
    pub(super) fn chunk(&self, buffer: BufferRef) -> &ProtocolObject<dyn MTLBuffer> {
        &self.frames[self.frame_index].chunks[buffer.buffer() as usize].buffer
    }

    /// What was let go of, kept by this frame until the GPU is done with it.
    pub(super) fn collect_retired(&mut self) {
        for retired in self.retired.take() {
            match retired {
                Retired::Texture(id) => {
                    if let Some(texture) = self.textures.remove(id) {
                        self.frames[self.frame_index].textures.push(texture.texture);
                    }
                }
                Retired::Pipeline(id) => {
                    if let Some(state) = self.maker.remove(id) {
                        self.frames[self.frame_index].pipelines.push(state);
                    }
                }
            }
        }
    }

    pub(super) fn finish_frame(&mut self, present: bool) {
        self.end_blit();
        self.collect_retired();
        if let Some(window) = self.window.take() {
            self.textures.remove(window);
        }
        let drawable = self.drawable.take();
        let commands = self.commands.take().expect("a frame begun");
        if present && let Some(drawable) = &drawable {
            commands.presentDrawable(ProtocolObject::from_ref(&**drawable));
        }
        commands.commit();
        let frame = &mut self.frames[self.frame_index];
        if let Some(label) = self.label.take() {
            frame.timed.push((label, commands.clone()));
        }
        frame.submitted = Some(commands);
        self.recorded = false;
        self.in_frame = false;
        self.maker.keep_pipelines();
    }
}
