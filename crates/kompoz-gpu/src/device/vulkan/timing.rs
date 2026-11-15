//! The GPU's times: timestamps around each pass, copy and mipmap chain of a timed frame, read once its fence is
//! signaled.

use super::VulkanDevice;
use crate::device::{GpuFrameTime, GpuPassTime};
use ash::vk;

/// A frame's timestamps at most: its passes' beyond go untimed.
pub(super) const TIMESTAMPS: u32 = 1024;
/// The last timestamp's label: the frame's end.
const END_OF_FRAME: &str = "end of frame";

impl VulkanDevice {
    pub(super) fn time_gpu(&mut self, on: bool) -> bool {
        if on && !self.choice.timestamps_valid {
            return false;
        }
        self.timing = on;
        if on {
            for frame in &mut self.frames {
                if frame.timestamps == vk::QueryPool::null() {
                    let info =
                        vk::QueryPoolCreateInfo::default().query_type(vk::QueryType::TIMESTAMP).query_count(TIMESTAMPS);
                    // SAFETY: a valid query pool info; destroyed in drop.
                    match unsafe { self.device.create_query_pool(&info, None) } {
                        Ok(pool) => frame.timestamps = pool,
                        Err(_) => {
                            self.timing = false;
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    /// A timestamp of this frame's, if timed: a pass's start (its label; only with room left for its end and the
    /// frame's), its end (empty), or the frame's end.
    pub(super) fn mark(&mut self, stage: vk::PipelineStageFlags, label: String) {
        let frame = &mut self.frames[self.frame_index];
        let room = if label == END_OF_FRAME {
            1
        } else if label.is_empty() {
            2
        } else {
            3
        };
        if !frame.timed || frame.marks.len() + room > TIMESTAMPS as usize {
            return;
        }
        // SAFETY: the frame's command buffer records; the query was reset at the frame's start and is in its pool.
        unsafe { self.device.cmd_write_timestamp(self.cmd, stage, frame.timestamps, frame.marks.len() as u32) };
        frame.marks.push(label);
    }

    pub(super) fn mark_end_of_frame(&mut self) {
        self.mark(vk::PipelineStageFlags::BOTTOM_OF_PIPE, END_OF_FRAME.to_string());
    }

    /// The frame's times, once its GPU work is done: each pass from its start to its end, the frame theirs together.
    pub(super) fn read_timestamps(&mut self, frame: usize) {
        let marks = std::mem::take(&mut self.frames[frame].marks);
        if marks.len() < 2 || marks.last().map(String::as_str) != Some(END_OF_FRAME) {
            return;
        }
        let mut t = vec![0u64; marks.len()];
        // SAFETY: every query read was written by the frame, whose fence is signaled.
        let read = unsafe {
            self.device.get_query_pool_results(self.frames[frame].timestamps, 0, &mut t, vk::QueryResultFlags::TYPE_64)
        };
        if read.is_err() {
            return;
        }
        let to_ms = self.choice.properties.limits.timestamp_period as f64 / 1e6;
        let mut time = GpuFrameTime::default();
        let mut start = 0;
        for i in 1..marks.len() - 1 {
            if !marks[i].is_empty() {
                start = i;
            } else if start != 0 {
                let ms = t[i].wrapping_sub(t[start]) as f64 * to_ms;
                time.passes.push(GpuPassTime { label: marks[start].clone(), ms });
                time.ms += ms;
                start = 0;
            }
        }
        if !time.passes.is_empty() {
            self.timed.push(time);
        }
    }
}
