//! The showcase's shots against the C++ Kompoz showcase's (stage 12's check: "the showcase's screenshots are
//! compared with the C++ version's"): both run headless on lavapipe with one `KOMPOZ_SCRIPT` and one environment
//! (no desktop accent, no wallpaper colors, Turkish, left to right), the Material tab shot page by page — light and
//! dark, at scales 1 and 2 —, the pages laid end to end and cut into the top app bar, the navigation bar and the
//! sections ([`cut`], [`parts`]) — and the places screen, opened by its deep link in a run of its own and shot once,
//! cut into its bars, its map and its sheet ([`run_parts`]) —, each told apart from the C++ one's with kompoz-gpu's golden difference (every
//! channel within 3), and what differs either listed with its reason in `reference/regions.txt` or reported.
//!
//! The C++ showcase draws its desktop menu bar (Dosya, Görünüm, Git: a 40 dp strip) above the app; KompozR's comes
//! with the window APIs (stage 15/16). So the C++ one runs in its 440 × 900 dp window and its pictures are kept
//! without the strip, and KompozR's runs 40 dp shorter (`resize 440 860` first; at scale 2, 880 × 1720 px): the app gets the same 440 × 860 dp
//! in both, and the script's positions are the same in the app (the C++ ones 40 dp lower in the window).
//!
//! Text is set in the same fonts in both: the C++ showcase reads Noto Sans (Regular, Medium, Bold) bundled next to
//! its binary and the system's Noto fallbacks (Mono, Arabic, Symbols 2) by path; KompozR reads its fonts through
//! fontconfig, here a configuration of only `fonts/` (the same files), so no other font of the machine's takes part.
//!
//! `cargo run -p showcase --release --bin showcase-shots -- compare` (or `reference --cpp <binary> --revision <rev>`
//! to write the C++ pictures anew), or `cargo xtask showcase-shots …`.

use kompoz::gpu::device::{Backend, NATIVE_BACKEND};
use kompoz::gpu::golden::{self, Difference};
use kompoz::graphics::{decode_png, save_png};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The app's size in dp (both showcases' window is 440 × 900; the C++ one's menu bar takes the top 40).
pub const APP_WIDTH: i32 = 440;
pub const APP_HEIGHT: i32 = 860;
/// The C++ showcase's menu bar (src/menu_bar.cpp's kBarHeight), above the app.
pub const MENU_BAR: i32 = 40;
/// How many pages the Material tab is shot in: the first, then one after each scroll, to its end.
pub const PAGES: usize = 24;
/// How far one scroll drags (dp): a finger down, moved up 50 dp a frame, held still, lifted (no fling).
pub const DRAG: i32 = 600;
/// Where it drags: in the screen's left margin (the sections' cards start 20 dp in), from 700 dp down the app.
pub const DRAG_X: i32 = 10;
pub const DRAG_FROM: i32 = 700;

/// One run of the showcase: a theme at a scale.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    pub dark: bool,
    pub scale: u32,
}

impl Config {
    /// All four: light and dark, at scales 1 and 2.
    pub fn all() -> [Config; 4] {
        [
            Config { dark: false, scale: 1 },
            Config { dark: true, scale: 1 },
            Config { dark: false, scale: 2 },
            Config { dark: true, scale: 2 },
        ]
    }

    /// Its directory's name: `light-1x`.
    pub fn name(self) -> String {
        format!("{}-{}x", if self.dark { "dark" } else { "light" }, self.scale)
    }
}

/// The places screen's address: both showcases open an address given on their command line (the C++ showcase's
/// `showcase kompoz://places`), as its NavHost's deep link.
pub const PLACES_LINK: &str = "kompoz://places";

/// Which showcase runs a script: positions in the app are the same; the C++ one's are `MENU_BAR` lower in its
/// window, and KompozR's window is made the app's size first.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Showcase {
    Cpp,
    KompozR,
}

/// The script of a run: the theme switched (the app bar's moon) for a dark one, the Material tab opened, a shot of
/// each page (`00.png` …) in the current directory.
pub fn script(config: Config, showcase: Showcase) -> String {
    let dy = if showcase == Showcase::Cpp { MENU_BAR } else { 0 };
    let mut s = String::new();
    // (Headless, a window's points are its pixels: at scale 2 it is made twice as many, to hold the same dp.)
    let k = config.scale as i32;
    let _ = write!(s, "resize {} {}; ", APP_WIDTH * k, (APP_HEIGHT + dy) * k);
    s.push_str("wait 10; ");
    if config.dark {
        let _ = write!(s, "click 316 {}; wait 40; ", 32 + dy);
    }
    let _ = write!(s, "click 385 {}; wait 60; shot 00.png; ", 830 + dy);
    for page in 1..PAGES {
        let from = DRAG_FROM + dy;
        let _ = write!(s, "down {DRAG_X} {from}; wait 1; ");
        let steps = DRAG / 50;
        for k in 1..=steps {
            let _ = write!(s, "move {DRAG_X} {}; wait 1; ", from - 50 * k);
        }
        let _ = write!(s, "wait 6; up {DRAG_X} {}; wait 60; shot {page:02}.png; ", from - DRAG);
    }
    s.push_str("quit");
    s
}

/// The script of the places run (the app opened at [`PLACES_LINK`]): the theme switched for a dark one, the screen
/// shot once its transition and the theme's have settled (`places.png`).
pub fn places_script(config: Config, showcase: Showcase) -> String {
    let dy = if showcase == Showcase::Cpp { MENU_BAR } else { 0 };
    let mut s = String::new();
    let k = config.scale as i32;
    let _ = write!(s, "resize {} {}; ", APP_WIDTH * k, (APP_HEIGHT + dy) * k);
    s.push_str("wait 10; ");
    if config.dark {
        let _ = write!(s, "click 316 {}; wait 40; ", 32 + dy);
    }
    s.push_str("wait 60; shot places.png; quit");
    s
}

/// The generic families of the app's text in [`fonts_dir`]'s fonts alone (`KOMPOZ_FONT_FAMILIES`), as
/// [`write_fontconfig`]'s configuration names them for the C++ showcase.
pub const FONT_FAMILIES: &str = "sans-serif=Noto Sans;serif=Noto Sans;cursive=Noto Sans;monospace=Noto Sans Mono";

/// The directory of the fonts both showcases set their text in (examples/showcase/fonts).
pub fn fonts_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fonts")
}

/// The directory of the C++ showcase's pictures (examples/showcase/reference).
pub fn reference_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("reference")
}

/// A fontconfig configuration of only the fonts in `fonts` (its cache in `dir`), written to `dir/fonts.conf`: the
/// families by name, sans-serif being Noto Sans and monospace Noto Sans Mono.
pub fn write_fontconfig(dir: &Path, fonts: &Path) -> std::io::Result<PathBuf> {
    let path = dir.join("fonts.conf");
    let alias = |generic: &str, family: &str| {
        format!(
            "  <alias binding=\"strong\"><family>{generic}</family><prefer><family>{family}</family></prefer></alias>\n"
        )
    };
    let conf = format!(
        "<?xml version=\"1.0\"?>\n<!DOCTYPE fontconfig SYSTEM \"urn:fontconfig:fonts.dtd\">\n<fontconfig>\n  <dir>{}</dir>\n  \
         <cachedir>{}</cachedir>\n{}{}{}</fontconfig>\n",
        fonts.display(),
        dir.join("fontconfig-cache").display(),
        alias("sans-serif", "Noto Sans"),
        alias("serif", "Noto Sans"),
        alias("monospace", "Noto Sans Mono"),
    );
    std::fs::write(&path, conf)?;
    Ok(path)
}

/// Runs `binary` headless with `args` and `script` in `dir` (its shots land there), the environment both share,
/// within two minutes. Its stderr, or why it failed.
pub fn run(
    binary: &Path,
    args: &[&str],
    dir: &Path,
    script: &str,
    config: Config,
    fontconfig: &Path,
) -> Result<String, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut command = Command::new(binary);
    command.args(args);
    // (on lavapipe, the CPU's Vulkan, unless the environment names a driver: every machine draws alike — on Windows
    // lavapipe registered as the system's driver, as CI's job does, since an elevated process's loader ignores
    // VK_DRIVER_FILES; where Metal is the API, on the system's GPU, whose pictures are lavapipe's within the goldens'
    // three, as kompoz-gpu's tests check)
    if NATIVE_BACKEND == Backend::Vulkan
        && !cfg!(windows)
        && std::env::var_os("VK_DRIVER_FILES").is_none()
        && std::env::var_os("VK_ICD_FILENAMES").is_none()
    {
        command.env("VK_DRIVER_FILES", lavapipe().ok_or("no lavapipe (/usr/share/vulkan/icd.d/lvp_icd*.json)")?);
    }
    let mut child = command
        .current_dir(dir)
        .env("KOMPOZ_HEADLESS", "1")
        .env("KOMPOZ_SCRIPT", script)
        .env("KOMPOZ_SCALE", config.scale.to_string())
        .env("KOMPOZ_NO_DESKTOP_ACCENT", "1")
        .env("KOMPOZ_NO_WALLPAPER_COLORS", "1")
        .env("KOMPOZ_LANGUAGE", "tr")
        .env("KOMPOZ_LAYOUT_DIRECTION", "ltr")
        // (the C++ showcase's text in fontconfig's configuration of the fonts; KompozR's in those fonts alone on
        // every system, whose font services — CoreText's, DirectWrite's — no configuration narrows)
        .env("FONTCONFIG_FILE", fontconfig)
        .env("KOMPOZ_FONTS", fonts_dir())
        .env("KOMPOZ_FONT_FAMILIES", FONT_FAMILIES)
        .env_remove("KOMPOZ_STATE_FILE")
        .env_remove("KOMPOZ_PREVIEW")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{}: {e}", binary.display()))?;
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut stderr = String::new();
    let reader = child.stderr.take().map(|mut e| {
        std::thread::spawn(move || {
            use std::io::Read;
            let mut s = String::new();
            let _ = e.read_to_string(&mut s);
            s
        })
    });
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{} didn't quit within two minutes", binary.display()));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    if let Some(r) = reader {
        stderr = r.join().unwrap_or_default();
    }
    if !status.success() {
        return Err(format!("{} failed ({status}):\n{stderr}", binary.display()));
    }
    Ok(stderr)
}

/// The runs of `showcase` (its `binary`) in `config` — the Material tab's pages, then the places screen —, shot into
/// `dir` (made anew), on lavapipe (where Vulkan is the API; else on the target's), checked for the validation layers'
/// errors.
pub fn shoot(binary: &Path, showcase: Showcase, config: Config, dir: &Path) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let fontconfig = write_fontconfig(dir, &fonts_dir()).map_err(|e| e.to_string())?;
    let runs = [(script(config, showcase), Vec::new()), (places_script(config, showcase), vec![PLACES_LINK])];
    for (script, args) in runs {
        let log = run(binary, &args, dir, &script, config, &fontconfig)?;
        if NATIVE_BACKEND == Backend::Vulkan && !log.contains("llvmpipe") {
            return Err(format!("{}: not drawn on lavapipe:\n{log}", config.name()));
        }
        let renderer = format!("kompoz: renderer: KompozR on {}", NATIVE_BACKEND.name());
        if !log.contains(&renderer) {
            return Err(format!("{}: not drawn with {}:\n{log}", config.name(), NATIVE_BACKEND.name()));
        }
        if log.contains("GPU validation") {
            return Err(format!("{}: validation errors:\n{log}", config.name()));
        }
    }
    Ok(())
}

/// Mesa's lavapipe's driver file.
fn lavapipe() -> Option<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir("/usr/share/vulkan/icd.d")
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("lvp_icd") && n.ends_with(".json"))
        })
        .collect();
    files.sort();
    files.into_iter().next()
}

/// A picture: RGBA, the top row first.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Picture {
    pub width: i32,
    pub height: i32,
    pub rgba: Vec<u8>,
}

impl Picture {
    pub fn load(path: &Path) -> Option<Picture> {
        let image = decode_png(&std::fs::read(path).ok()?)?;
        Some(Picture { width: image.width, height: image.height, rgba: image.rgba })
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        save_png(path, self.width, self.height, &self.rgba).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Without its top `rows` rows.
    pub fn without_top(&self, rows: i32) -> Picture {
        let start = (rows * self.width * 4) as usize;
        Picture { width: self.width, height: self.height - rows, rgba: self.rgba[start..].to_vec() }
    }
}

impl Picture {
    /// Rows `from..to` of it.
    pub fn rows(&self, from: i32, to: i32) -> Picture {
        let (from, to) = (from.clamp(0, self.height), to.clamp(0, self.height));
        let w = (self.width * 4) as usize;
        Picture { width: self.width, height: to - from, rgba: self.rgba[from as usize * w..to as usize * w].to_vec() }
    }

    fn row(&self, y: i32) -> &[u8] {
        let w = (self.width * 4) as usize;
        &self.rgba[y as usize * w..(y as usize + 1) * w]
    }

    fn pixel(&self, x: i32, y: i32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }

    /// `below` appended under it (of its width).
    fn append(&mut self, below: &Picture) {
        self.rgba.extend_from_slice(&below.rgba);
        self.height += below.height;
    }
}

/// The app's bars, dp: the top app bar over the content on the first page (it scrolls away after), the navigation
/// bar under it on every page.
pub const TOP_BAR: i32 = 64;
pub const BOTTOM_BAR: i32 = 64;

/// A run's pictures cut as they are compared: the top app bar and the navigation bar (from the first page), and
/// the Material tab's content in one picture, the pages laid end to end (each page's rows past the last one's,
/// found where its rows match the last page's best: what a scroll moved can't be known otherwise, and animated
/// parts make the match not quite whole).
pub struct Cut {
    pub top: Picture,
    pub bottom: Picture,
    pub content: Picture,
}

/// The pages `pages` of a run at `scale` cut so.
pub fn cut(pages: &[Picture], scale: u32) -> Cut {
    let k = scale as i32;
    let first = &pages[0];
    // (the navigation bar's top: up from the window's bottom, its color in the left margin as far as it goes, at
    // least as far as the bar's height)
    let bar = first.pixel(2 * k, first.height - 1);
    let mut band_end = first.height - BOTTOM_BAR * k;
    while band_end > 0 && first.pixel(2 * k, band_end - 1) == bar {
        band_end -= 1;
    }
    let top = first.rows(0, TOP_BAR * k);
    let bottom = first.rows(band_end, first.height);
    let mut content = first.rows(TOP_BAR * k, band_end);
    // (the last page's rows above the navigation bar)
    let mut last = first.rows(0, band_end);
    for page in &pages[1..] {
        let band = page.rows(0, band_end);
        // the scroll: how far up the last page's rows moved, the shift its rows match best at
        // (only rows unlike the row above them count: runs of like rows — a card's empty rows — match anywhere)
        let telling: Vec<bool> = (0..band_end).map(|y| y > 0 && band.row(y) != band.row(y - 1)).collect();
        let mut best = (0usize, 0i32);
        for shift in 0..band_end {
            let overlap = band_end - shift;
            let mut same = 0;
            for y in 0..overlap {
                same += (telling[y as usize] && band.row(y) == last.row(y + shift)) as usize;
            }
            if same > best.0 {
                best = (same, shift);
            }
        }
        let shift = best.1;
        if shift > 0 {
            // the rows it brings: past the last page's end
            content.append(&band.rows(band_end - shift, band_end));
        }
        last = band;
    }
    Cut { top, bottom, content }
}

/// The sections in a run's content: where the cards are (dp rows of `scale` px), told by the screen's background
/// between them in the left margin (x 10 dp) and in a card's padding (x 28 dp) alike.
pub fn sections(content: &Picture, scale: u32) -> Vec<(i32, i32)> {
    let k = scale as i32;
    let background = content.pixel(10 * k, 0);
    let gap = |y: i32| content.pixel(10 * k, y) == background && content.pixel(28 * k, y) == background;
    let mut out = Vec::new();
    let mut start = None;
    for y in 0..content.height {
        match (gap(y), start) {
            (false, None) => start = Some(y),
            (true, Some(s)) => {
                out.push((s, y));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, content.height));
    }
    out
}

/// What is compared of a run: its bars and each section (`top`, `bottom`, `s00` … in the content's order).
pub fn parts(cut: &Cut, scale: u32) -> Vec<(String, Picture)> {
    let mut out = vec![("top".to_string(), cut.top.clone()), ("bottom".to_string(), cut.bottom.clone())];
    for (i, (from, to)) in sections(&cut.content, scale).into_iter().enumerate() {
        out.push((format!("s{i:02}"), cut.content.rows(from, to)));
    }
    out
}

/// The section names, in the screen's order (MaterialScreen's calls), for the report.
pub const SECTION_NAMES: [&str; 27] = [
    "SearchSection",
    "TooltipSection",
    "DropdownSection",
    "DatePickerSection",
    "TimePickerSection",
    "InteractionSection",
    "AnchoredDraggableSection",
    "RippleSection",
    "SelectableRowsSection",
    "SemanticsSection",
    "DynamicColorSection",
    "InsetsSection",
    "ImeNestedScrollSection",
    "FrameRateSection",
    "TextFieldsSection",
    "AnimateItemSection",
    "ButtonsSection",
    "ChipsSection",
    "SelectionControlsSection",
    "TabsSection",
    "ListItemSection",
    "DialogsSection",
    "FabMenuSection",
    "ShortNavigationBarSection",
    "WideNavigationRailSection",
    "FloatingToolbarsSection",
    "CardsSection",
];

/// The pages a run shot into `dir` (`00.png` …).
pub fn load_pages(dir: &Path) -> Result<Vec<Picture>, String> {
    (0..PAGES)
        .map(|p| {
            let path = dir.join(format!("{p:02}.png"));
            Picture::load(&path).ok_or_else(|| format!("no shot {}", path.display()))
        })
        .collect()
}

/// The places screen's shot of a run in `dir` (`places.png`).
pub fn load_places(dir: &Path) -> Result<Picture, String> {
    let path = dir.join("places.png");
    Picture::load(&path).ok_or_else(|| format!("no shot {}", path.display()))
}

/// The places screen's sheet: how far it peeks (PlacesScreen's `sheetPeekHeight`), and the rows above it its
/// shadow may reach, dp.
pub const PLACES_PEEK: i32 = 128;
pub const PLACES_SHADOW: i32 = 8;

/// What is compared of `showcase`'s runs shot into `dir` at `scale` (the C++ showcase's pictures without its menu
/// bar): the Material tab cut ([`cut`]) and its [`parts`], then the places screen's page cut as the first page is,
/// into `places-top`, `places-map` (the map above the sheet), `places-sheet` (the peeking sheet: the content's last
/// rows, so that it is compared where it lies — on the navigation bar's top, wherever that is) and `places-bottom`.
pub fn run_parts(dir: &Path, showcase: Showcase, scale: u32) -> Result<(Cut, Vec<(String, Picture)>), String> {
    let menu_bar = if showcase == Showcase::Cpp { MENU_BAR * scale as i32 } else { 0 };
    let pages: Vec<Picture> = load_pages(dir)?.iter().map(|p| p.without_top(menu_bar)).collect();
    let material = cut(&pages, scale);
    let mut out = parts(&material, scale);
    let places = cut(&[load_places(dir)?.without_top(menu_bar)], scale);
    let sheet_top = places.content.height - (PLACES_PEEK + PLACES_SHADOW) * scale as i32;
    out.push(("places-top".to_string(), places.top));
    out.push(("places-map".to_string(), places.content.rows(0, sheet_top)));
    out.push(("places-sheet".to_string(), places.content.rows(sheet_top, places.content.height)));
    out.push(("places-bottom".to_string(), places.bottom));
    Ok((material, out))
}

/// A run's parts written into `dir` (`top.png`, `bottom.png`, `s00.png` …): the C++ pictures as kept.
pub fn save_parts(parts: &[(String, Picture)], dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (name, p) in parts {
        p.save(&dir.join(format!("{name}.png")))?;
    }
    Ok(())
}

/// A place where the showcases differ for a reason: the config it holds for (a name, or every one: `*`), the part
/// (`top`, `bottom`, `s00` …, or `*`), its rectangle in dp in the part, and why.
#[derive(Clone, PartialEq, Debug)]
pub struct Region {
    pub config: String,
    pub part: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// A height of its own: the part's height may differ from the picture's (`config part height reason…`).
    pub tall: bool,
    pub reason: String,
}

impl Region {
    fn holds_for(&self, config: &str, part: &str) -> bool {
        (self.config == "*" || self.config == config) && (self.part == "*" || self.part == part)
    }

    /// Whether it holds the pixel (px) at `scale`.
    fn contains(&self, px: i32, py: i32, scale: u32) -> bool {
        let s = scale as f32;
        let (x, y) = (px as f32 + 0.5, py as f32 + 0.5);
        x >= self.x * s && x < (self.x + self.width) * s && y >= self.y * s && y < (self.y + self.height) * s
    }
}

/// The regions of `reference/regions.txt`: a line `config part x y width height reason…` each (dp; `#` starts a
/// comment).
pub fn read_regions(path: &Path) -> Result<Vec<Region>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut regions = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let at = |m: String| format!("{}:{}: {m}", path.display(), n + 1);
        if words.len() >= 4 && words[2] == "height" {
            regions.push(Region {
                config: words[0].to_string(),
                part: words[1].to_string(),
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
                tall: true,
                reason: words[3..].join(" "),
            });
            continue;
        }
        if words.len() < 7 {
            return Err(at("a region is `config part x y width height reason`".to_string()));
        }
        let number = |w: &str| w.parse::<f32>().map_err(|e| at(e.to_string()));
        regions.push(Region {
            config: words[0].to_string(),
            part: words[1].to_string(),
            x: number(words[2])?,
            y: number(words[3])?,
            width: number(words[4])?,
            height: number(words[5])?,
            tall: false,
            reason: words[6..].join(" "),
        });
    }
    Ok(regions)
}

/// How a part came out against its picture.
#[derive(Clone, PartialEq, Debug)]
pub struct Comparison {
    pub config: String,
    pub part: String,
    /// Outside the listed regions (of the rows both have): what must be nothing.
    pub outside: Difference,
    /// In each listed region that holds for it: how much differs there (accepted, with its reason).
    pub inside: Vec<(Region, Difference)>,
    /// Its height against the picture's, px, where they differ.
    pub heights: Option<(i32, i32)>,
    /// Why its height may differ (a `height` region's reason).
    pub height_accepted: Option<String>,
    /// Why it couldn't be compared.
    pub problem: Option<String>,
}

/// Compares a run's `parts` with the pictures in `reference/<config>/`, the regions left out; writes each part into
/// `out` (`<part>.png`), and for one that differs outside them `<part>.diff.png` and the picture (`<part>.cpp.png`).
pub fn compare(
    config: Config,
    parts: &[(String, Picture)],
    reference: &Path,
    regions: &[Region],
    out: &Path,
) -> Vec<Comparison> {
    let name = config.name();
    let _ = std::fs::create_dir_all(out);
    let mut result = Vec::new();
    let mut names: Vec<String> = parts.iter().map(|(n, _)| n.clone()).collect();
    // (a part the reference has and the run doesn't: reported too)
    if let Ok(entries) = std::fs::read_dir(reference.join(&name)) {
        let mut theirs: Vec<String> =
            entries.filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".png").map(str::to_string)).collect();
        theirs.sort();
        for t in theirs {
            if !names.contains(&t) {
                names.push(t);
            }
        }
    }
    for part in names {
        let mut c = Comparison {
            config: name.clone(),
            part: part.clone(),
            outside: Difference::default(),
            inside: Vec::new(),
            heights: None,
            height_accepted: None,
            problem: None,
        };
        let theirs = Picture::load(&reference.join(&name).join(format!("{part}.png")));
        let ours = parts.iter().find(|(n, _)| *n == part).map(|(_, p)| p.clone());
        match (theirs, ours) {
            (None, _) => c.problem = Some("no C++ picture of it".to_string()),
            (_, None) => c.problem = Some("not in KompozR's shots".to_string()),
            (Some(a), Some(b)) if a.width != b.width => {
                c.problem = Some(format!("{} px wide against {}", b.width, a.width));
            }
            (Some(a), Some(b)) => {
                let held: Vec<&Region> = regions.iter().filter(|g| g.holds_for(&name, &part) && !g.tall).collect();
                if a.height != b.height {
                    c.heights = Some((b.height, a.height));
                    c.height_accepted =
                        regions.iter().find(|g| g.holds_for(&name, &part) && g.tall).map(|g| g.reason.clone());
                }
                // (the part as shot, beside the comparison: what a region list is made from)
                let _ = b.save(&out.join(format!("{part}.png")));
                let h = a.height.min(b.height);
                let (a, b) = (a.rows(0, h), b.rows(0, h));
                let w = a.width;
                let at = |i: usize| ((i as i32) % w, (i as i32) / w);
                c.outside = golden::difference_within(&a.rgba, &b.rgba, tolerance(), |i| {
                    let (x, y) = at(i);
                    held.iter().any(|g| g.contains(x, y, config.scale))
                });
                for g in &held {
                    let d = golden::difference_within(&a.rgba, &b.rgba, tolerance(), |i| {
                        let (x, y) = at(i);
                        !g.contains(x, y, config.scale)
                    });
                    c.inside.push(((*g).clone(), d));
                }
                if c.outside.count > 0 || c.heights.is_some() {
                    let diff = Picture { width: a.width, height: h, rgba: golden::diff_picture(&a.rgba, &b.rgba) };
                    let _ = diff.save(&out.join(format!("{part}.diff.png")));
                    let _ = a.save(&out.join(format!("{part}.cpp.png")));
                }
            }
        }
        result.push(c);
    }
    result
}

/// How far a channel may be off the C++ showcase's pictures (drawn on lavapipe): the goldens' [`golden::TOLERANCE`]
/// drawn on lavapipe too; where Metal is the API, on the system's GPU, twice it — its pictures within the goldens'
/// three of lavapipe's (kompoz-gpu's tests check it), lavapipe's within three of the C++ ones (an Apple GPU's edge
/// coverage and blending land a channel 4 off, never more, on the macOS runner).
pub fn tolerance() -> i32 {
    if NATIVE_BACKEND == Backend::Metal { 2 * golden::TOLERANCE } else { golden::TOLERANCE }
}

/// A part's name with its section's (`s05 InteractionSection`).
pub fn part_title(part: &str) -> String {
    match part.strip_prefix('s').and_then(|n| n.parse::<usize>().ok()) {
        Some(i) => format!("{part} {}", SECTION_NAMES.get(i).copied().unwrap_or("?")),
        None => part.to_string(),
    }
}

/// The report of a comparison, a line a part: its height if it differs, what differs outside the regions, and in
/// each region.
pub fn report(comparisons: &[Comparison]) -> String {
    let mut s = String::new();
    for c in comparisons {
        let _ = write!(s, "{}/{}: ", c.config, part_title(&c.part));
        if let Some(p) = &c.problem {
            let _ = writeln!(s, "{p}");
            continue;
        }
        if let Some((ours, theirs)) = c.heights {
            match &c.height_accepted {
                Some(why) => {
                    let _ = write!(s, "{ours} px tall against {theirs} (accepted: {why}); ");
                }
                None => {
                    let _ = write!(s, "{ours} px tall against {theirs}; ");
                }
            }
        }
        let _ = writeln!(s, "{} pixels differ outside the regions (by up to {})", c.outside.count, c.outside.worst);
        for (g, d) in &c.inside {
            if d.count > 0 {
                let _ = writeln!(
                    s,
                    "    {} px in {} {} {}×{} (accepted: {})",
                    d.count, g.x, g.y, g.width, g.height, g.reason
                );
            }
        }
    }
    s
}

/// Whether every part matched its picture outside the regions (and is as tall).
pub fn all_match(comparisons: &[Comparison]) -> bool {
    comparisons
        .iter()
        .all(|c| c.problem.is_none() && (c.heights.is_none() || c.height_accepted.is_some()) && c.outside.count == 0)
}
