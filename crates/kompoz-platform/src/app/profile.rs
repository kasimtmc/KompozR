//! `KOMPOZ_PROFILE` (C++'s profileFrame and profileGpu): where frame time goes. Each slow frame is logged (input
//! handling, compose and layout, waits, drawing, present; nodes, layers, paths and shadows drawn and kept, textures
//! made, glyphs rasterized, how much was drawn again), and a summary every second; with the GPU's timing, a frame
//! that took the GPU longer than the display's period with its costliest passes.

use kompoz_gpu::device::GpuFrameTime;

/// What a frame's drawing counted (the renderer's and the display list's counters).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DrawCounts {
    pub drawn_nodes: usize,
    pub kept_nodes: usize,
    pub layers: i32,
    pub kept_layers: i32,
    pub paths: i32,
    pub kept_paths: i32,
    pub path_shadows: i32,
    pub kept_shadows: i32,
    pub textures_made: i32,
    pub glyphs: usize,
}

/// The profile's running totals (C++'s AppState::Profile).
#[derive(Clone, Debug, Default)]
pub(crate) struct Profile {
    last_start: u64,
    /// Input handled since the last frame.
    pub events_ns: u64,
    summary_start: u64,
    /// When the last frame shown was handed to the display.
    last_shown: u64,
    /// When frames started being due (busy since).
    pub due_since: u64,
    /// The last iteration's frame asked for the next (frames meant to follow each other).
    pub busy: bool,
    frames: i32,
    slow: i32,
    /// Recorded, nothing on the screen differed.
    unchanged: i32,
    shown: i32,
    /// Display refreshes between frames shown back to back.
    missed: i32,
    work_total: f64,
    work_max: f64,
    wait_total: f64,
    interval_max: f64,
    /// Fractions of the screen drawn again, summed.
    drawn_area: f64,
    compose_total: f64,
    draw_total: f64,
    record_total: f64,
    /// This frame's recording (drawFrame).
    pub record_ns: u64,
    /// And its wait for an image of the window to draw into (a wait, not work).
    pub acquire_ns: u64,
    gpu_frames: i32,
    gpu_total: f64,
    gpu_max: f64,
}

fn ms(ns: u64) -> f64 {
    ns as f64 / 1e6
}

impl Profile {
    /// A frame: started at `start`, composed by `composed`, `waited` for the GPU, drawn by `drawn`, presented by
    /// `presented` (all ns); whether `shown`; `area` drawn again. Logs it if slow, and the summary each second.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        log: &dyn Fn(&str),
        period_ns: u64,
        start: u64,
        composed: u64,
        waited: u64,
        drawn: u64,
        presented: u64,
        shown: bool,
        area: f32,
        counts: DrawCounts,
    ) {
        let period = ms(period_ns);
        let interval = if self.last_start > 0 { ms(start.saturating_sub(self.last_start)) } else { 0.0 };
        let acquire = ms(self.acquire_ns);
        let events = ms(self.events_ns);
        let compose = ms(composed.saturating_sub(start));
        let wait = ms(waited) + acquire;
        let draw = ms(drawn.saturating_sub(composed).saturating_sub(waited)) - acquire;
        let present = ms(presented.saturating_sub(drawn));
        let work = events + compose + draw;
        self.last_start = start;
        self.events_ns = 0;
        if self.summary_start == 0 {
            self.summary_start = start;
        }
        self.frames += 1;
        self.work_total += work;
        self.compose_total += compose;
        self.draw_total += draw;
        self.record_total += ms(self.record_ns);
        self.record_ns = 0;
        self.acquire_ns = 0;
        self.work_max = self.work_max.max(work);
        self.wait_total += wait;
        self.interval_max = self.interval_max.max(interval);
        if area == 0.0 {
            self.unchanged += 1;
            self.due_since = presented; // (the refreshes until it had nothing new to show: none missed)
        }
        self.drawn_area += if area < 0.0 { 1.0 } else { area as f64 };
        if shown {
            // Refreshes of the display that went by without a frame, between two shown back to back (the app busy
            // all along: not those it waited through for an event).
            if self.last_shown > 0 && self.busy {
                let gap = ms(presented.saturating_sub(self.last_shown.max(self.due_since)));
                self.missed += ((gap / period).round() as i32 - 1).max(0);
            }
            self.last_shown = presented;
            self.shown += 1;
        }
        if work > period || interval > 2.0 * period + 1.0 {
            self.slow += 1;
            let c = counts;
            log(&format!(
                "kompoz: frame {interval:.1} ms after the last: input {events:.1}, compose+layout {compose:.1}, wait \
                 {wait:.1}, draw {draw:.1}, present {present:.1} ms; {} nodes drawn ({} kept), {} layers ({} kept), {} \
                 paths ({} kept), {} path shadows ({} kept), {} textures made, {} glyphs, {:.0}% drawn again",
                c.drawn_nodes,
                c.kept_nodes,
                c.layers,
                c.kept_layers,
                c.paths,
                c.kept_paths,
                c.path_shadows,
                c.kept_shadows,
                c.textures_made,
                c.glyphs,
                (if area < 0.0 { 1.0 } else { area }) * 100.0
            ));
        }
        if start.saturating_sub(self.summary_start) >= 1_000_000_000 {
            let frames = self.frames as f64;
            let gpu = if self.gpu_frames > 0 {
                format!(", GPU avg {:.1} max {:.1} ms", self.gpu_total / self.gpu_frames as f64, self.gpu_max)
            } else {
                String::new()
            };
            log(&format!(
                "kompoz: {} frames in {:.0} ms ({} shown, {} refreshes missed; {} changed nothing on the screen; {:.0}% \
                 of it drawn again on average), work avg {:.1} max {:.1} ms (compose+layout {:.1}, draw {:.1}, of it \
                 recording {:.1}), wait avg {:.1} ms{gpu}, longest gap {:.1} ms, {} slow",
                self.frames,
                ms(start - self.summary_start),
                self.shown,
                self.missed,
                self.unchanged,
                self.drawn_area / frames * 100.0,
                self.work_total / frames,
                self.work_max,
                self.compose_total / frames,
                self.draw_total / frames,
                self.record_total / frames,
                self.wait_total / frames,
                self.interval_max,
                self.slow
            ));
            let (last_shown, busy, due_since) = (self.last_shown, self.busy, self.due_since);
            *self =
                Profile { last_start: start, summary_start: start, last_shown, busy, due_since, ..Profile::default() };
        }
    }

    /// The GPU's times for the frames it has done: into the summary, and one that took longer than the display's
    /// period logged with its costliest passes.
    pub fn gpu(&mut self, log: &dyn Fn(&str), period_ns: u64, times: Vec<GpuFrameTime>) {
        let period = ms(period_ns);
        for mut t in times {
            self.gpu_frames += 1;
            self.gpu_total += t.ms;
            self.gpu_max = self.gpu_max.max(t.ms);
            if t.ms <= period {
                continue;
            }
            t.passes.sort_by(|a, b| b.ms.total_cmp(&a.ms));
            let mut passes: String =
                t.passes.iter().take(6).map(|p| format!("{} {:.1}", p.label, p.ms)).collect::<Vec<_>>().join(", ");
            if t.passes.len() > 6 {
                passes += &format!(" (+{} more)", t.passes.len() - 6);
            }
            log(&format!("kompoz: GPU frame {:.1} ms{}{passes}", t.ms, if passes.is_empty() { "" } else { ": " }));
        }
    }
}
