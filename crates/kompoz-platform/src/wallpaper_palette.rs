//! A wallpaper's palette, the way Android 12 takes its colors from the wallpaper (C++'s common/wallpaper_colors.cpp):
//! the picture read, decoded as big as a screen (decoders scale down as they go) and its palette picked
//! ([`DynamicTonalPalette::from_image`]) — what Linux's and Windows' desktop services share, each finding the
//! wallpaper and following its changes its own way (`linux::wallpaper`, `windows::wallpaper`).
//!
//! A palette is worked out on a thread of its own ([`PaletteWork`]), which stopping (the app quitting) never waits
//! for: decoding a big picture is one long call (seconds in a debug build), so it is told to stop at its next step
//! and left to end by itself.

use kompoz_graphics::ImageRequestSize;
use kompoz_graphics::material_color::palettes::DynamicTonalPalette;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The wallpaper's palette, if it is a picture that can be read.
pub fn palette_of(path: &str) -> Option<DynamicTonalPalette> {
    palette_unless_stopped(path, &AtomicBool::new(false))
}

/// [`palette_of`], given up (none) at the step after `stop` is set: read, decoded, made ARGB, its palette picked.
pub fn palette_unless_stopped(path: &str, stop: &AtomicBool) -> Option<DynamicTonalPalette> {
    let go = || !stop.load(Ordering::SeqCst);
    let bytes = std::fs::read(path).ok().filter(|b| !b.is_empty() && go())?;
    palette_of_bytes(&bytes, stop)
}

/// A picture's palette from its file's bytes (any format our decoders read: Windows' TranscodedWallpaper has no
/// extension), given up at the step after `stop` is set.
pub fn palette_of_bytes(bytes: &[u8], stop: &AtomicBool) -> Option<DynamicTonalPalette> {
    let go = || !stop.load(Ordering::SeqCst);
    let image = kompoz_graphics::decode_image(bytes, ImageRequestSize::new(1920, 1080, true)).filter(|_| go())?;
    let rgba = image.pixels();
    let argb: Vec<u32> = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| u32::from(p[3]) << 24 | u32::from(p[0]) << 16 | u32::from(p[1]) << 8 | u32::from(p[2]))
        .collect();
    go().then(|| DynamicTonalPalette::from_image(&argb, image.width() as usize, image.height() as usize))
}

/// A palette being worked out on a thread of its own, told to stop when dropped (its result then goes nowhere):
/// `done` is called on that thread when it is ready (to wake whoever waits for it), never after it was dropped.
pub struct PaletteWork {
    stop: Arc<AtomicBool>,
    /// The palette once worked out (none in it: the picture couldn't be read).
    result: Arc<Mutex<Option<Option<DynamicTonalPalette>>>>,
}

impl PaletteWork {
    pub fn start(path: String, done: impl Fn() + Send + Sync + 'static) -> PaletteWork {
        let work = PaletteWork { stop: Arc::new(AtomicBool::new(false)), result: Arc::new(Mutex::new(None)) };
        let (result, done) = (work.result.clone(), Arc::new(done));
        let finish = move |palette| {
            *result.lock().unwrap_or_else(|e| e.into_inner()) = Some(palette);
            done();
        };
        let spawned = std::thread::Builder::new().name("kompoz-wallpaper-palette".into()).spawn({
            let (path, stop, finish) = (path.clone(), work.stop.clone(), finish.clone());
            move || {
                let palette = palette_unless_stopped(&path, &stop);
                if !stop.load(Ordering::SeqCst) {
                    finish(palette);
                }
            }
        });
        if spawned.is_err() {
            finish(palette_unless_stopped(&path, &work.stop)); // (no thread to be had: here, then)
        }
        work
    }

    /// The palette, once it was worked out.
    pub fn take(&self) -> Option<Option<DynamicTonalPalette>> {
        self.result.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}

impl Drop for PaletteWork {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// A 24-bit BMP of one color (`rgb`), `w` × `h` (the tests' wallpapers).
#[cfg(test)]
pub(crate) fn test_bmp(rgb: u32, w: u32, h: u32) -> Vec<u8> {
    let row = (w * 3).div_ceil(4) * 4;
    let size = 54 + row * h;
    let mut b = vec![0u8; size as usize];
    let mut put = |at: usize, v: u32, n: usize| {
        for i in 0..n {
            b[at + i] = (v >> (8 * i)) as u8;
        }
    };
    put(0, u32::from(b'B') | u32::from(b'M') << 8, 2);
    put(2, size, 4);
    put(10, 54, 4);
    put(14, 40, 4);
    put(18, w, 4);
    put(22, h, 4);
    put(26, 1, 2);
    put(28, 24, 2);
    for y in 0..h {
        for x in 0..w {
            put((54 + y * row + x * 3) as usize, rgb, 3); // (little endian: B, G, R)
        }
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use kompoz_graphics::Color;
    use kompoz_graphics::material_color::hct::Hct;

    /// A picture's palette is its color's (as Android's from its best seed), from any decoder's format; none for
    /// what isn't a picture, or once stopped.
    #[test]
    fn a_picture_gives_its_palette() {
        let hue_of = |p: &DynamicTonalPalette| Hct::from_color(p.accent1[8]).hue();
        for rgb in [0x2E7D32, 0x1565C0, 0xD32F2F] {
            let palette = palette_of_bytes(&test_bmp(rgb, 64, 48), &AtomicBool::new(false)).expect("a palette");
            let seed = DynamicTonalPalette::from_seed(Color::hex(rgb));
            assert!((hue_of(&palette) - hue_of(&seed)).abs() < 8.0, "{rgb:06x}");
        }
        assert_eq!(palette_of_bytes(b"not a picture", &AtomicBool::new(false)), None);
        assert_eq!(palette_of_bytes(&test_bmp(0x2E7D32, 8, 8), &AtomicBool::new(true)), None);
        assert_eq!(palette_of("/surely/not/a/wallpaper.png"), None);
    }

    /// The work's result comes on its thread, `done` called once it is there.
    #[test]
    fn the_work_tells_when_done() {
        let path = std::env::temp_dir().join(format!("kompoz-palette-{}.bmp", std::process::id()));
        std::fs::write(&path, test_bmp(0x1565C0, 32, 32)).unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        let work = PaletteWork::start(path.to_string_lossy().into_owned(), move || {
            let _ = sender.send(());
        });
        receiver.recv_timeout(std::time::Duration::from_secs(60)).expect("done");
        assert!(work.take().expect("worked out").is_some());
        assert_eq!(work.take(), None, "taken once");
        let _ = std::fs::remove_file(path);
    }
}
