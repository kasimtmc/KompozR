//! Scenes against their pictures in tests/golden, as the C++ Kompoz's `matchesGolden`: every pixel within 3 of the
//! picture in every channel (GPUs round a little differently). What differs is written beside the picture to look
//! at: `<name>.actual.png`, and `<name>.diff.png` (the pixels off by more than 3 in red over the picture, faded).
//!
//!   KOMPOZ_GOLDEN_UPDATE=1   writes the pictures anew (look at them before committing them)
//!   KOMPOZ_GOLDEN_DIR=<dir>  the pictures from there

use crate::harness::{Pixels, max_difference};
use kompoz_graphics::{decode_png, save_png};
use std::path::{Path, PathBuf};

/// Where the pictures are: `KOMPOZ_GOLDEN_DIR`, else the crate's tests/golden.
pub fn golden_dir() -> PathBuf {
    std::env::var_os("KOMPOZ_GOLDEN_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("golden"))
}

/// `KOMPOZ_GOLDEN_UPDATE=1`: the pictures written anew.
pub fn updating() -> bool {
    std::env::var_os("KOMPOZ_GOLDEN_UPDATE").is_some_and(|v| v == "1")
}

/// A PNG file's pixels, as they are stored (the pictures are read-back pixels, premultiplied, written as they are).
pub fn load_png(path: &Path) -> Option<Pixels> {
    let image = decode_png(&std::fs::read(path).ok()?)?;
    Some(Pixels { width: image.width, height: image.height, rgba: image.rgba })
}

pub fn save(path: &Path, pixels: &Pixels) -> bool {
    save_png(path, pixels.width, pixels.height, &pixels.rgba).is_ok()
}

/// How two pictures of a size differ: the largest channel difference, and how many pixels differ by more than 3.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Difference {
    pub worst: i32,
    pub count: usize,
}

/// None if they aren't of a size.
pub fn difference(a: &Pixels, b: &Pixels) -> Option<Difference> {
    if (a.width, a.height) != (b.width, b.height) {
        return None;
    }
    let mut d = Difference::default();
    for (p, q) in a.rgba.as_chunks::<4>().0.iter().zip(b.rgba.as_chunks::<4>().0) {
        let m = max_difference(*p, *q);
        d.worst = d.worst.max(m);
        d.count += (m > 3) as usize;
    }
    Some(d)
}

/// The picture faded, the pixels of `actual` off by more than 3 in red.
pub fn diff_picture(golden: &Pixels, actual: &Pixels) -> Pixels {
    let mut out = golden.clone();
    for (o, (g, a)) in out
        .rgba
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(golden.rgba.as_chunks::<4>().0.iter().zip(actual.rgba.as_chunks::<4>().0))
    {
        *o = if max_difference(*g, *a) > 3 {
            [255, 0, 0, 255]
        } else {
            [g[0] / 4 + 191, g[1] / 4 + 191, g[2] / 4 + 191, 255]
        };
    }
    out
}

/// What matching a picture came to.
#[derive(Clone, PartialEq, Debug)]
pub enum Outcome {
    Matches(Difference),
    /// Written anew (updating).
    Written,
    /// To be written anew, but it couldn't be.
    Unwritable,
    /// No picture of that size there.
    NoPicture,
    /// Differs: by this much, the pictures written beside it.
    Differs(Difference),
}

fn beside(dir: &Path, name: &str, suffix: &str) -> PathBuf {
    dir.join(format!("{name}{suffix}.png"))
}

/// `actual` against the picture `name` in `dir` (written anew if `update`), what differs written beside it.
pub fn match_golden_in(dir: &Path, name: &str, actual: &Pixels, update: bool) -> Outcome {
    let path = beside(dir, name, "");
    if update {
        return if save(&path, actual) { Outcome::Written } else { Outcome::Unwritable };
    }
    let Some(golden) = load_png(&path) else { return Outcome::NoPicture };
    let Some(d) = difference(&golden, actual) else { return Outcome::NoPicture };
    if d.count == 0 {
        return Outcome::Matches(d);
    }
    save(&beside(dir, name, ".actual"), actual);
    save(&beside(dir, name, ".diff"), &diff_picture(&golden, actual));
    Outcome::Differs(d)
}

/// The scene against its picture (a check).
#[allow(dead_code)] // (the renderer's scenes': their tests come with it)
pub fn matches_golden(name: &str, actual: &Pixels) {
    match match_golden_in(&golden_dir(), name, actual, updating()) {
        Outcome::Matches(_) => check!(true),
        Outcome::Written => println!("  wrote {}", beside(&golden_dir(), name, "").display()),
        Outcome::Unwritable => check!(false, "  {name}: its picture couldn't be written"),
        Outcome::NoPicture => check!(false, "  {name}: no picture of that size (KOMPOZ_GOLDEN_UPDATE=1 writes it)"),
        Outcome::Differs(d) => {
            check!(false, "  {name}: {} pixels differ by more than 3 (by up to {})", d.count, d.worst)
        }
    }
}
