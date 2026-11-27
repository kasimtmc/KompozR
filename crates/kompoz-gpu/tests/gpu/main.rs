//! Tests of the GPU layer on a real device, as the C++ Kompoz's tests/gpu_tests.cpp: one device made without a
//! window (drawing only into textures: lavapipe in CI), every test run on it in sequence on this thread, then the
//! tests that make devices of their own, one after another; each check counted, the run failing if any failed.
//! What is known exactly is checked as such; scenes are compared with their pictures in tests/golden.
//!
//!   KOMPOZ_GPU_REQUIRED=1        no device is a failure (CI), not a skip
//!   KOMPOZ_GPU_VALIDATION=1      the validation layer on: any error it reports is a failure
//!   KOMPOZ_GPU_DEVICE=<part>     the GPU whose name has it ("Intel", "RTX", "llvmpipe")
//!   KOMPOZ_GOLDEN_UPDATE=1       writes the pictures anew (look at them before committing them)
//!   KOMPOZ_GOLDEN_DIR=<dir>      the pictures from there
//!
//! On lavapipe, as CI runs it:
//!   VK_DRIVER_FILES=$(ls /usr/share/vulkan/icd.d/lvp_icd*.json | head -1) KOMPOZ_GPU_VALIDATION=1 \
//!   KOMPOZ_GPU_REQUIRED=1 cargo test -p kompoz-gpu --test gpu
//! Arguments name the tests to run (those whose names have one of them): `cargo test -p kompoz-gpu --test gpu --
//! readback`.
//!
//! In a browser, on WebGPU (tests/gpu/web.rs): `cargo xtask web-gpu-test [--firefox] [names…]` builds this for the
//! web and runs it in headless Chrome (SwiftShader's WebGPU) or Firefox, the variables above and the names
//! (`KOMPOZ_GPU_TESTS=a,b`) in the page's query.

use std::sync::atomic::{AtomicUsize, Ordering};

static CHECKS: AtomicUsize = AtomicUsize::new(0);
static FAILURES: AtomicUsize = AtomicUsize::new(0);

/// A line of the run's output: printed natively; on the web the page's (the web's std prints nowhere).
macro_rules! println {
    () => {
        $crate::say(String::new())
    };
    ($($arg:tt)*) => {
        $crate::say(format!($($arg)*))
    };
}

fn say(line: String) {
    #[cfg(not(target_family = "wasm"))]
    std::println!("{line}");
    #[cfg(target_family = "wasm")]
    kompoz_web::log(kompoz_web::Level::Info, &line);
}

/// A check, counted; where it fails, what failed (and the message, if given) is printed and the run fails.
macro_rules! check {
    ($condition:expr) => {
        check!($condition, "")
    };
    ($condition:expr, $($message:tt)+) => {{
        $crate::CHECKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let held: bool = $condition;
        if !held {
            $crate::FAILURES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            println!("  {}:{}: check!({}) failed", file!(), line!(), stringify!($condition));
            let message = format!($($message)+);
            if !message.is_empty() {
                println!("{message}");
            }
        }
    }};
}

mod device;
mod exact;
mod files;
mod fonts;
mod frames;
mod golden;
mod harness;
mod images;
mod layers;
mod scenes;
mod stretch;
mod submit;
#[cfg(target_family = "wasm")]
mod web;

use harness::Gpu;
use kompoz_gpu::device::Device;

/// Tests on the one device.
const ON_THE_DEVICE: &[(&str, fn(&mut Gpu))] = &[
    ("textures_read_back_as_uploaded", device::textures_read_back_as_uploaded),
    ("readbacks_leave_the_frame_going", device::readbacks_leave_the_frame_going),
    ("gpu_times_come_back", device::gpu_times_come_back),
    ("pass_areas_clear_only_there", device::pass_areas_clear_only_there),
    ("discarded_textures_are_drawn_over", device::discarded_textures_are_drawn_over),
    ("scissors_stay_within_the_pass", device::scissors_stay_within_the_pass),
    ("pipelines_are_made_as_described", device::pipelines_are_made_as_described),
    ("pipelines_are_made_on_other_threads", device::pipelines_are_made_on_other_threads),
    ("mipmaps_halve_the_texture", device::mipmaps_halve_the_texture),
    ("copies_move_texels", device::copies_move_texels),
    ("frame_memory_grows_as_asked", device::frame_memory_grows_as_asked),
    ("textures_come_and_go", device::textures_come_and_go),
    ("edges_are_covered_exactly", exact::edges_are_covered_exactly),
    ("edges_cut_pixels_exactly", exact::edges_cut_pixels_exactly),
    ("clips_and_translations_are_exact", exact::clips_and_translations_are_exact),
    ("paths_are_covered_exactly", exact::paths_are_covered_exactly),
    ("strokes_cover_once", exact::strokes_cover_once),
    ("gradients_match_color_at", exact::gradients_match_color_at),
    ("blend_modes_match_blend", exact::blend_modes_match_blend),
    ("color_filters_match_apply", exact::color_filters_match_apply),
    ("target_copies_fit_every_target", exact::target_copies_fit_every_target),
    ("transformed_edges_are_exact", exact::transformed_edges_are_exact),
    ("clips_cut_as_their_shapes", exact::clips_cut_as_their_shapes),
    ("conics_are_exact", exact::conics_are_exact),
    ("images_are_sampled_as_asked", images::images_are_sampled_as_asked),
    ("shaders_paint_as_they_are", images::shaders_paint_as_they_are),
    ("meshes_draw_as_skia_does", images::meshes_draw_as_skia_does),
    ("bitmaps_are_drawn_into_as_android_does", images::bitmaps_are_drawn_into_as_android_does),
    ("bitmaps_drawn_into_in_a_frame_wait_their_turn", images::bitmaps_drawn_into_in_a_frame_wait_their_turn),
    ("vectors_draw_as_compose", images::vectors_draw_as_compose),
    ("aliased_drawing_is_skias", exact::aliased_drawing_is_skias),
    ("path_effects_apply_where_drawn", exact::path_effects_apply_where_drawn),
    ("variants_fall_back_to_the_whole_shader", submit::variants_fall_back_to_the_whole_shader),
    ("pipelines_are_warmed_from_their_list", submit::pipelines_are_warmed_from_their_list),
    ("drawn_twice_in_a_frame_alike", submit::drawn_twice_in_a_frame_alike),
    ("paths_share_their_masks", submit::paths_share_their_masks),
    ("a_path_keeps_its_coverage", submit::a_path_keeps_its_coverage),
    ("transforms_carry_everything", frames::transforms_carry_everything),
    ("primitives_draw_alone_as_among_others", frames::primitives_draw_alone_as_among_others),
    ("kept_frames_move_what_scrolled", frames::kept_frames_move_what_scrolled),
    ("blit_copies_textures_as_they_are", frames::blit_copies_textures_as_they_are),
    ("text_is_drawn_from_the_atlas", frames::text_is_drawn_from_the_atlas),
    ("scenes_match_their_pictures", scenes::scenes_match_their_pictures),
    ("layer_alpha_is_exact", layers::layer_alpha_is_exact),
    ("graphics_layers_composite_as_compose", layers::graphics_layers_composite_as_compose),
    ("blurs_are_gaussian", layers::blurs_are_gaussian),
    ("path_shadows_are_composes", layers::path_shadows_are_composes),
    ("elevation_shadows_are_androids", layers::elevation_shadows_are_androids),
    ("kept_layers_draw_as_drawn_afresh", layers::kept_layers_draw_as_drawn_afresh),
    ("a_layer_is_kept_from_its_first_frame", layers::a_layer_is_kept_from_its_first_frame),
    ("path_shadows_are_kept", layers::path_shadows_are_kept),
    ("layer_textures_come_in_few_sizes", layers::layer_textures_come_in_few_sizes),
    ("stretched_layers_are_hwuis", stretch::stretched_layers_are_hwuis),
    ("stretch_comes_after_the_render_effect", stretch::stretch_comes_after_the_render_effect),
    ("stretched_layers_are_kept_as_the_stretch_changes", stretch::stretched_layers_are_kept_as_the_stretch_changes),
    ("stretched_layers_match_their_picture", stretch::stretched_layers_match_their_picture),
];

/// Tests that make devices of their own, one after another (after the one device is gone), or need none.
const ON_THEIR_OWN: &[(&str, fn())] = &[
    ("pipelines_are_kept_between_runs", device::pipelines_are_kept_between_runs),
    ("goldens_compare_as_they_should", device::goldens_compare_as_they_should),
];

fn run<T>(name: &str, test: impl FnOnce() -> T) {
    let failures = FAILURES.load(Ordering::Relaxed);
    let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(test));
    if ran.is_err() {
        FAILURES.fetch_add(1, Ordering::Relaxed);
    }
    let failed = ran.is_err() || FAILURES.load(Ordering::Relaxed) != failures;
    println!("{} {name}", if failed { "FAILED" } else { "ok    " });
}

fn main() {
    #[cfg(target_family = "wasm")]
    kompoz_web::start();
    // (Natively the arguments; on the web the page's KOMPOZ_GPU_TESTS=a,b.)
    #[cfg(not(target_family = "wasm"))]
    let filters: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    #[cfg(target_family = "wasm")]
    let filters: Vec<String> = kompoz_runtime::environment::var("KOMPOZ_GPU_TESTS")
        .map(|names| names.split(',').filter(|n| !n.is_empty()).map(str::to_string).collect())
        .unwrap_or_default();
    let selected = |name: &str| filters.is_empty() || filters.iter().any(|f| name.contains(f.as_str()));
    match Gpu::start(None) {
        Err(why) => {
            println!("skipped: {why}");
            if kompoz_gpu::device::gpu_required() {
                FAILURES.fetch_add(1, Ordering::Relaxed);
            }
            return finish();
        }
        Ok(mut g) => {
            println!("device: {}", g.device.device_name());
            for (name, test) in ON_THE_DEVICE.iter().filter(|(name, _)| selected(name)) {
                run(name, || test(&mut g));
            }
            // (Counted with KOMPOZ_GPU_VALIDATION=1; WebGPU always validates.)
            let errors = g.device.validation_errors();
            check!(errors == 0, "  {errors} validation errors");
        }
    }
    for (name, test) in ON_THEIR_OWN.iter().filter(|(name, _)| selected(name)) {
        run(name, test);
    }
    finish();
}

/// The checks and failures said, and the run's exit status theirs (on the web: told the page, which has no exit).
fn finish() {
    let (checks, failures) = (CHECKS.load(Ordering::Relaxed), FAILURES.load(Ordering::Relaxed));
    println!("{checks} checks, {failures} failed");
    #[cfg(not(target_family = "wasm"))]
    std::process::exit(if failures == 0 { 0 } else { 1 });
    #[cfg(target_family = "wasm")]
    web::finish(checks, failures);
}
