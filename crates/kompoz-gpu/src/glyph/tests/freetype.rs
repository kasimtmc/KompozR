//! Parity with FreeType: the glyph bitmaps the C++ Kompoz's font system made with FreeType 2.15 (dumped by
//! testdata/freetype_reference.c into testdata/freetype-reference.bin) against ours of the same glyphs.

use crate::glyph::bitmap::{BitmapImage, bitmap_glyph};
use crate::glyph::blur::blurred;
use crate::glyph::cache::Face;
use crate::glyph::color::paint_colr;
use crate::glyph::outline::{GlyphStroke, GlyphStyle, Hinting, scaler, styled_coverage};
use kompoz_graphics::{StrokeCap, StrokeJoin};
use skrifa::ScalerFont;
use std::collections::HashMap;

const REFERENCE: &[u8] = include_bytes!("../testdata/freetype-reference.bin");

/// A reference bitmap: what was drawn (the dumper's name), where, and its pixels (1 or 4 channels).
struct Record {
    name: String,
    channels: usize,
    left: i32,
    top: i32,
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

/// Reads the records' fields in turn.
struct Reader(&'static [u8]);

impl Reader {
    fn take(&mut self, n: usize) -> &'static [u8] {
        let (s, rest) = self.0.split_at(n);
        self.0 = rest;
        s
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take(4).try_into().unwrap())
    }
}

fn records() -> Vec<Record> {
    let mut out = Vec::new();
    let mut r = Reader(REFERENCE);
    while !r.0.is_empty() {
        let channels = r.take(1)[0] as usize;
        let len = u16::from_le_bytes(r.take(2).try_into().unwrap()) as usize;
        let name = String::from_utf8(r.take(len).to_vec()).unwrap();
        let left = r.u32() as i32;
        let top = r.u32() as i32;
        let width = r.u32() as usize;
        let height = r.u32() as usize;
        let pixels = r.take(width * height * channels).to_vec();
        out.push(Record { name, channels, left, top, width, height, pixels });
    }
    out
}

/// Ours of a record, as (left, top, width, height, pixels) in the record's channels.
type Image = (i32, i32, usize, usize, Vec<u8>);

struct Fonts(HashMap<String, Face>);

impl Fonts {
    fn face(&mut self, path: &str) -> &mut Face {
        self.0.entry(path.to_string()).or_insert_with(|| {
            let bytes = std::fs::read(format!("{}/{path}", env!("CARGO_MANIFEST_DIR"))).expect("a test font");
            Face::new(std::sync::Arc::new(bytes), 0, &[]).expect("a font")
        })
    }
}

fn ours(fonts: &mut Fonts, record: &Record) -> Option<Image> {
    let words: Vec<&str> = record.name.split(' ').collect();
    let (mode, path) = (words[0], words[1]);
    let px: u32 = words[2].parse().unwrap();
    let c = char::from_u32(u32::from_str_radix(words[3], 16).unwrap()).unwrap();
    let face = fonts.face(path);
    let glyph = face.font().charmap().map_unicode(c).expect("the character's glyph").to_u32();
    let style = |w: &[&str]| -> GlyphStyle {
        let stroke_px: u32 = w[4].parse().unwrap();
        let join = match w[5] {
            "r" => StrokeJoin::Round,
            "b" => StrokeJoin::Bevel,
            _ => StrokeJoin::Miter,
        };
        GlyphStyle {
            scale_x: w[7].parse().unwrap(),
            // (The dumper's is FreeType's matrix's, y up: x moves by skew · y. Android's, ours, is the other way.)
            skew_x: -w[8].parse::<f32>().unwrap(),
            stroke: (stroke_px > 0).then(|| GlyphStroke {
                width: stroke_px,
                cap: StrokeCap::Butt,
                join,
                miter: w[6].parse().unwrap(),
                path_effect: None,
            }),
            ..GlyphStyle::PLAIN
        }
    };
    let coverage = |c: crate::glyph::Coverage| Some((c.left, c.top, c.width, c.height, c.alpha));
    match mode {
        "light" => {
            let outline = face.outline(px, Hinting::Light, glyph)?;
            coverage(styled_coverage(outline, px, &GlyphStyle::PLAIN, 1.0, 0))
        }
        "styled" => {
            let outline = face.outline(px, Hinting::Light, glyph)?;
            coverage(styled_coverage(outline, px, &style(&words), 1.0, 0))
        }
        "blur" => {
            let outline = face.outline(px, Hinting::Light, glyph)?;
            let radius: u32 = words[9].parse().unwrap();
            coverage(blurred(&styled_coverage(outline, px, &style(&words), 1.0, 0), radius))
        }
        "colr" => {
            let font = face.font().clone();
            let layers = scaler(&font, px, Hinting::Normal);
            let units = ScalerFont::unhinted(font.clone(), None);
            let bytes = std::fs::read(format!("{}/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap();
            let glyf = skrifa::FontRef::new(&bytes).ok();
            let painted = paint_colr(&font, glyph, px, [0, 0, 0, 255], &layers, glyf.as_ref(), &units)?;
            let image = painted.image?;
            Some((image.left, image.top, image.width, image.height, image.rgba))
        }
        "bitmap" => match bitmap_glyph(face.font(), glyph, px)? {
            BitmapImage::Color(image) => Some((image.left, image.top, image.width, image.height, image.rgba)),
            BitmapImage::Coverage(c) => coverage(c),
        },
        _ => panic!("unknown mode {mode}"),
    }
}

/// How two images differ: the largest channel difference over the union of their boxes (outside a box: 0), and how
/// many channels differ by more than 3.
fn compare(record: &Record, ours: &Image) -> (u8, usize) {
    let (left, top, width, height, ref pixels) = *ours;
    let n = record.channels;
    let x0 = record.left.min(left);
    let x1 = (record.left + record.width as i32).max(left + width as i32);
    let y1 = record.top.max(top);
    let y0 = (record.top - record.height as i32).min(top - height as i32);
    let at = |l: i32, t: i32, w: usize, h: usize, px: &[u8], x: i32, y: i32, c: usize| -> u8 {
        let (cx, cy) = (x - l, t - 1 - y);
        if cx < 0 || cy < 0 || cx as usize >= w || cy as usize >= h {
            return 0;
        }
        px[(cy as usize * w + cx as usize) * n + c]
    };
    let (mut max, mut over) = (0u8, 0usize);
    for y in y0..y1 {
        for x in x0..x1 {
            for c in 0..n {
                let a = at(record.left, record.top, record.width, record.height, &record.pixels, x, y, c);
                let b = at(left, top, width, height, pixels, x, y, c);
                let d = a.abs_diff(b);
                max = max.max(d);
                over += (d > 3) as usize;
            }
        }
    }
    (max, over)
}

/// The characters FreeType 2.14 hints with blue zones ignored below their base (a cedilla: afadjust.c's
/// AF_IGNORE_*_BOTTOM), inside its hinter, before edges are fitted: skrifa's hinter, which predates that, fits their
/// cedillas to the descender zone (see `adjust`).
const BLUES_IGNORED: [&str; 4] = ["E7", "15F", "C7", "15E"];

/// What a record is, for the totals: the mode, and for styled glyphs whether stroked.
fn kind(name: &str) -> &'static str {
    let words: Vec<&str> = name.split(' ').collect();
    match words[0] {
        "light" if BLUES_IGNORED.contains(&words[3]) => "light, blue zones ignored below (FreeType 2.14)",
        "light" => "light",
        "styled" if words[4] != "0" => "stroked (FT_Stroker)",
        "styled" => "stretched or slanted",
        "blur" if words[4] != "0" => "blurred, stroked",
        "blur" => "blurred",
        "colr" => "COLR v0",
        "bitmap" => "CBDT bitmap",
        other => panic!("unknown mode {other}"),
    }
}

/// The C++ Kompoz's FreeType glyphs against ours. Printed: each kind's count, how many are drawn exactly, the
/// largest channel difference, how many channels differ by more than 3, how many boxes differ.
#[test]
#[cfg_attr(miri, ignore)]
fn glyphs_are_freetypes() {
    let mut fonts = Fonts(HashMap::new());
    let mut stats: HashMap<&str, (usize, usize, u8, usize, usize)> = HashMap::new();
    let records = records();
    assert_eq!(records.len(), 496);
    for r in &records {
        let kind = kind(&r.name);
        let ours = ours(&mut fonts, r).unwrap_or((0, 0, 0, 0, Vec::new()));
        let (max, over) = compare(r, &ours);
        let same_box = (ours.0, ours.1, ours.2, ours.3) == (r.left, r.top, r.width, r.height);
        let e = stats.entry(kind).or_default();
        e.0 += 1;
        e.1 += (max == 0 && same_box) as usize;
        e.2 = e.2.max(max);
        e.3 += over;
        e.4 += (!same_box) as usize;
        if max > 3 || !same_box {
            let (b, theirs) = ((ours.0, ours.1, ours.2, ours.3), (r.left, r.top, r.width, r.height));
            println!("{}: largest difference {max}, {over} over 3, box {b:?} (FreeType's {theirs:?})", r.name);
        }
    }
    let mut kinds: Vec<_> = stats.iter().collect();
    kinds.sort();
    for (kind, (n, exact, max, over, boxes)) in &kinds {
        println!(
            "{kind}: {n} glyphs, {exact} exact, largest difference {max}, {over} channels over 3, {boxes} boxes differ"
        );
    }
    // Exact: light hinted outlines (their diacritics moved as FreeType 2.14 moves them), stretched and slanted
    // ones, blurred ones, COLR v0 layers, CBDT bitmaps.
    for kind in ["light", "stretched or slanted", "blurred", "COLR v0", "CBDT bitmap"] {
        let (n, exact, ..) = stats[kind];
        assert_eq!(exact, n, "{kind}");
    }
    // Not exact, by known causes (what is left is checked not to grow): cedillas hinted by skrifa's older hinter;
    // strokes outlined as kompoz-graphics outlines them (straight pieces, Skia's joins: a miter past its limit
    // beveled where FT_Stroker clips it) where FreeType offsets the curves themselves.
    let (_, _, _, over, _) = stats["light, blue zones ignored below (FreeType 2.14)"];
    assert!(over <= 482, "{over}");
    let (_, _, _, over, boxes) = stats["stroked (FT_Stroker)"];
    assert!(over <= 434 && boxes <= 3, "{over}, {boxes}");
    let (_, _, max, ..) = stats["blurred, stroked"];
    assert!(max <= 3, "{max}");
}
