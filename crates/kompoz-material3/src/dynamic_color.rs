//! Color schemes of the system's colors, as Compose's DynamicTonalPalette.android.kt (Android 12's "Material You",
//! `dynamicLightColorScheme(context)`): the system's palettes ([`LocalDynamicTonalPalette`], what the host is told —
//! on a Linux desktop made from its accent color) become a [`TonalPalette`] as Android 12–13's resources give it
//! (the tones the system has as they are; the others the nearest one's hue and chroma at that tone, Compose's
//! `setLuminance`), then a scheme of Compose's mapping (`dynamicLightColorScheme31`). Where the system gives no
//! palette the scheme is Material's baseline. The wallpaper's own palettes ([`LocalWallpaperTonalPalette`], where
//! the platform reads the wallpaper) make schemes of their own the same way (the C++ Kompoz's).
//!
//! Android 14's own scheme colors (`system_primary_light`, …: `dynamicLightColorScheme34`) are Android's resources:
//! the Android platform's (stage 15).

use crate::color_scheme::{ColorScheme, dark_color_scheme, light_color_scheme};
use crate::tonal_palette::TonalPalette;
use kompoz_graphics::Color;
use kompoz_graphics::material_color::hct::HctSolver;
use kompoz_graphics::material_color::palettes::DynamicTonalPalette;
use kompoz_ui::platform::{LocalDynamicTonalPalette, LocalWallpaperTonalPalette};

/// Material's light scheme of the system's colors (Compose's `dynamicLightColorScheme(context)`): from the host's
/// palette ([`LocalDynamicTonalPalette`]), Material's baseline where it has none ([`is_dynamic_color_available`]).
pub fn dynamic_light_color_scheme() -> ColorScheme {
    match LocalDynamicTonalPalette.current() {
        Some(p) => dynamic_light_color_scheme_of(&p),
        None => light_color_scheme(),
    }
}

/// Material's dark scheme of the system's colors (Compose's `dynamicDarkColorScheme(context)`), the baseline's
/// where the host has none.
pub fn dynamic_dark_color_scheme() -> ColorScheme {
    match LocalDynamicTonalPalette.current() {
        Some(p) => dynamic_dark_color_scheme_of(&p),
        None => dark_color_scheme(),
    }
}

/// Whether the host has the system's palette (the dynamic schemes aren't the baseline's).
pub fn is_dynamic_color_available() -> bool {
    LocalDynamicTonalPalette.current().is_some()
}

/// The light scheme of a palette as Android's resources give it (Compose's `dynamicLightColorScheme31`).
///
/// Kotlin: `dynamicLightColorScheme(palette: DynamicTonalPalette)`
pub fn dynamic_light_color_scheme_of(palette: &DynamicTonalPalette) -> ColorScheme {
    dynamic_light_color_scheme31(&dynamic_tonal_palette(palette))
}

/// The dark scheme of a palette (Compose's `dynamicDarkColorScheme31`).
///
/// Kotlin: `dynamicDarkColorScheme(palette: DynamicTonalPalette)`
pub fn dynamic_dark_color_scheme_of(palette: &DynamicTonalPalette) -> ColorScheme {
    dynamic_dark_color_scheme31(&dynamic_tonal_palette(palette))
}

/// Material's light scheme of the wallpaper's colors: from the host's wallpaper palette
/// ([`LocalWallpaperTonalPalette`]), the baseline where it has none ([`is_wallpaper_color_available`]).
pub fn wallpaper_light_color_scheme() -> ColorScheme {
    match LocalWallpaperTonalPalette.current() {
        Some(p) => dynamic_light_color_scheme_of(&p),
        None => light_color_scheme(),
    }
}

/// Material's dark scheme of the wallpaper's colors, the baseline's where the host has none.
pub fn wallpaper_dark_color_scheme() -> ColorScheme {
    match LocalWallpaperTonalPalette.current() {
        Some(p) => dynamic_dark_color_scheme_of(&p),
        None => dark_color_scheme(),
    }
}

/// Whether the host has the wallpaper's palette.
pub fn is_wallpaper_color_available() -> bool {
    LocalWallpaperTonalPalette.current().is_some()
}

/// The system's palettes as Android 12–13's color resources give them (Compose's internal `dynamicTonalPalette`):
/// `system_<key>_<n>` is tone `100 - n / 10` of the key color (`_0` white, `_1000` black); the neutral tones the
/// resources lack are tone 40 (`_600`) at that lightness.
pub(crate) fn dynamic_tonal_palette(p: &DynamicTonalPalette) -> TonalPalette {
    TonalPalette {
        neutral100: p.neutral1[0],
        neutral99: p.neutral1[1],
        neutral98: set_luminance(p.neutral1[8], 98.0),
        neutral96: set_luminance(p.neutral1[8], 96.0),
        neutral95: p.neutral1[2],
        neutral94: set_luminance(p.neutral1[8], 94.0),
        neutral92: set_luminance(p.neutral1[8], 92.0),
        neutral90: p.neutral1[3],
        neutral87: set_luminance(p.neutral1[8], 87.0),
        neutral80: p.neutral1[4],
        neutral70: p.neutral1[5],
        neutral60: p.neutral1[6],
        neutral50: p.neutral1[7],
        neutral40: p.neutral1[8],
        neutral30: p.neutral1[9],
        neutral24: set_luminance(p.neutral1[8], 24.0),
        neutral22: set_luminance(p.neutral1[8], 22.0),
        neutral20: p.neutral1[10],
        neutral17: set_luminance(p.neutral1[8], 17.0),
        neutral12: set_luminance(p.neutral1[8], 12.0),
        neutral10: p.neutral1[11],
        neutral6: set_luminance(p.neutral1[8], 6.0),
        neutral4: set_luminance(p.neutral1[8], 4.0),
        neutral0: p.neutral1[12],
        neutral_variant100: p.neutral2[0],
        neutral_variant99: p.neutral2[1],
        neutral_variant98: set_luminance(p.neutral2[8], 98.0),
        neutral_variant96: set_luminance(p.neutral2[8], 96.0),
        neutral_variant95: p.neutral2[2],
        neutral_variant94: set_luminance(p.neutral2[8], 94.0),
        neutral_variant92: set_luminance(p.neutral2[8], 92.0),
        neutral_variant90: p.neutral2[3],
        neutral_variant87: set_luminance(p.neutral2[8], 87.0),
        neutral_variant80: p.neutral2[4],
        neutral_variant70: p.neutral2[5],
        neutral_variant60: p.neutral2[6],
        neutral_variant50: p.neutral2[7],
        neutral_variant40: p.neutral2[8],
        neutral_variant30: p.neutral2[9],
        neutral_variant24: set_luminance(p.neutral2[8], 24.0),
        neutral_variant22: set_luminance(p.neutral2[8], 22.0),
        neutral_variant20: p.neutral2[10],
        neutral_variant17: set_luminance(p.neutral2[8], 17.0),
        neutral_variant12: set_luminance(p.neutral2[8], 12.0),
        neutral_variant10: p.neutral2[11],
        neutral_variant6: set_luminance(p.neutral2[8], 6.0),
        neutral_variant4: set_luminance(p.neutral2[8], 4.0),
        neutral_variant0: p.neutral2[12],
        primary100: p.accent1[0],
        primary99: p.accent1[1],
        primary95: p.accent1[2],
        primary90: p.accent1[3],
        primary80: p.accent1[4],
        primary70: p.accent1[5],
        primary60: p.accent1[6],
        primary50: p.accent1[7],
        primary40: p.accent1[8],
        primary30: p.accent1[9],
        primary20: p.accent1[10],
        primary10: p.accent1[11],
        primary0: p.accent1[12],
        secondary100: p.accent2[0],
        secondary99: p.accent2[1],
        secondary95: p.accent2[2],
        secondary90: p.accent2[3],
        secondary80: p.accent2[4],
        secondary70: p.accent2[5],
        secondary60: p.accent2[6],
        secondary50: p.accent2[7],
        secondary40: p.accent2[8],
        secondary30: p.accent2[9],
        secondary20: p.accent2[10],
        secondary10: p.accent2[11],
        secondary0: p.accent2[12],
        tertiary100: p.accent3[0],
        tertiary99: p.accent3[1],
        tertiary95: p.accent3[2],
        tertiary90: p.accent3[3],
        tertiary80: p.accent3[4],
        tertiary70: p.accent3[5],
        tertiary60: p.accent3[6],
        tertiary50: p.accent3[7],
        tertiary40: p.accent3[8],
        tertiary30: p.accent3[9],
        tertiary20: p.accent3[10],
        tertiary10: p.accent3[11],
        tertiary0: p.accent3[12],
    }
}

/// The light scheme of a tonal palette (Compose's internal `dynamicLightColorScheme31`): the baseline's error colors, the rest the
/// palette's.
pub(crate) fn dynamic_light_color_scheme31(tonal_palette: &TonalPalette) -> ColorScheme {
    ColorScheme {
        primary: tonal_palette.primary40,
        on_primary: tonal_palette.primary100,
        primary_container: tonal_palette.primary90,
        on_primary_container: tonal_palette.primary10,
        inverse_primary: tonal_palette.primary80,
        secondary: tonal_palette.secondary40,
        on_secondary: tonal_palette.secondary100,
        secondary_container: tonal_palette.secondary90,
        on_secondary_container: tonal_palette.secondary10,
        tertiary: tonal_palette.tertiary40,
        on_tertiary: tonal_palette.tertiary100,
        tertiary_container: tonal_palette.tertiary90,
        on_tertiary_container: tonal_palette.tertiary10,
        background: tonal_palette.neutral_variant98,
        on_background: tonal_palette.neutral_variant10,
        surface: tonal_palette.neutral_variant98,
        on_surface: tonal_palette.neutral_variant10,
        surface_variant: tonal_palette.neutral_variant90,
        on_surface_variant: tonal_palette.neutral_variant30,
        inverse_surface: tonal_palette.neutral_variant20,
        inverse_on_surface: tonal_palette.neutral_variant95,
        outline: tonal_palette.neutral_variant50,
        outline_variant: tonal_palette.neutral_variant80,
        scrim: tonal_palette.neutral_variant0,
        surface_bright: tonal_palette.neutral_variant98,
        surface_dim: tonal_palette.neutral_variant87,
        surface_container: tonal_palette.neutral_variant94,
        surface_container_high: tonal_palette.neutral_variant92,
        surface_container_highest: tonal_palette.neutral_variant90,
        surface_container_low: tonal_palette.neutral_variant96,
        surface_container_lowest: tonal_palette.neutral_variant100,
        surface_tint: tonal_palette.primary40,
        primary_fixed: tonal_palette.primary90,
        primary_fixed_dim: tonal_palette.primary80,
        on_primary_fixed: tonal_palette.primary10,
        on_primary_fixed_variant: tonal_palette.primary30,
        secondary_fixed: tonal_palette.secondary90,
        secondary_fixed_dim: tonal_palette.secondary80,
        on_secondary_fixed: tonal_palette.secondary10,
        on_secondary_fixed_variant: tonal_palette.secondary30,
        tertiary_fixed: tonal_palette.tertiary90,
        tertiary_fixed_dim: tonal_palette.tertiary80,
        on_tertiary_fixed: tonal_palette.tertiary10,
        on_tertiary_fixed_variant: tonal_palette.tertiary30,
        ..light_color_scheme()
    }
}

/// The dark scheme of a tonal palette (Compose's internal `dynamicDarkColorScheme31`): the baseline's error colors, the rest the
/// palette's.
pub(crate) fn dynamic_dark_color_scheme31(tonal_palette: &TonalPalette) -> ColorScheme {
    ColorScheme {
        primary: tonal_palette.primary80,
        on_primary: tonal_palette.primary20,
        primary_container: tonal_palette.primary30,
        on_primary_container: tonal_palette.primary90,
        inverse_primary: tonal_palette.primary40,
        secondary: tonal_palette.secondary80,
        on_secondary: tonal_palette.secondary20,
        secondary_container: tonal_palette.secondary30,
        on_secondary_container: tonal_palette.secondary90,
        tertiary: tonal_palette.tertiary80,
        on_tertiary: tonal_palette.tertiary20,
        tertiary_container: tonal_palette.tertiary30,
        on_tertiary_container: tonal_palette.tertiary90,
        background: tonal_palette.neutral_variant6,
        on_background: tonal_palette.neutral_variant90,
        surface: tonal_palette.neutral_variant6,
        on_surface: tonal_palette.neutral_variant90,
        surface_variant: tonal_palette.neutral_variant30,
        on_surface_variant: tonal_palette.neutral_variant80,
        inverse_surface: tonal_palette.neutral_variant90,
        inverse_on_surface: tonal_palette.neutral_variant20,
        outline: tonal_palette.neutral_variant60,
        outline_variant: tonal_palette.neutral_variant30,
        scrim: tonal_palette.neutral_variant0,
        surface_bright: tonal_palette.neutral_variant24,
        surface_dim: tonal_palette.neutral_variant6,
        surface_container: tonal_palette.neutral_variant12,
        surface_container_high: tonal_palette.neutral_variant17,
        surface_container_highest: tonal_palette.neutral_variant22,
        surface_container_low: tonal_palette.neutral_variant10,
        surface_container_lowest: tonal_palette.neutral_variant4,
        surface_tint: tonal_palette.primary80,
        primary_fixed: tonal_palette.primary90,
        primary_fixed_dim: tonal_palette.primary80,
        on_primary_fixed: tonal_palette.primary10,
        on_primary_fixed_variant: tonal_palette.primary30,
        secondary_fixed: tonal_palette.secondary90,
        secondary_fixed_dim: tonal_palette.secondary80,
        on_secondary_fixed: tonal_palette.secondary10,
        on_secondary_fixed_variant: tonal_palette.secondary30,
        tertiary_fixed: tonal_palette.tertiary90,
        tertiary_fixed_dim: tonal_palette.tertiary80,
        on_tertiary_fixed: tonal_palette.tertiary10,
        on_tertiary_fixed_variant: tonal_palette.tertiary30,
        ..dark_color_scheme()
    }
}

/// The color at another lightness: `new_luminance`'s L* at this color's CAM16 hue and chroma (Compose's internal
/// `Color.setLuminance`, Compose's float CAM of it then HctSolver's color; the ends, under 0.0001 or over 99.9999,
/// the grey of that L*).
pub(crate) fn set_luminance(color: Color, new_luminance: f32) -> Color {
    if !(0.0001..=99.9999).contains(&new_luminance) {
        return Color::from_argb(argb_from_lstar(new_luminance as f64));
    }
    let (hue, chroma) = cam_hue_chroma(color_to_argb(color));
    Color::from_argb(HctSolver::solve_to_int(hue as f64, chroma as f64, new_luminance as f64))
}

/// A color's ARGB (Compose's toArgb: its sRGB channels rounded).
fn color_to_argb(color: Color) -> u32 {
    let c = color.in_srgb();
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    byte(c.alpha()) << 24 | byte(c.red()) << 16 | byte(c.green()) << 8 | byte(c.blue())
}

/// The grey of an L*, as Compose's color utilities make it (`CamUtils.argbFromLstar`: through XYZ).
fn argb_from_lstar(lstar: f64) -> u32 {
    let fy = (lstar + 16.0) / 116.0;
    let kappa = 24389.0 / 27.0;
    let epsilon = 216.0 / 24389.0;
    let y = if lstar > 8.0 { fy * fy * fy } else { lstar / kappa };
    let cube_exceeds_epsilon = fy * fy * fy > epsilon;
    let x = if cube_exceeds_epsilon { fy * fy * fy } else { lstar / kappa };
    let z = if cube_exceeds_epsilon { fy * fy * fy } else { lstar / kappa };
    let w = WHITE_POINT_D65;
    let (x, y, z) = (x * w[0] as f64, y * w[1] as f64, z * w[2] as f64);
    let m = XYZ_TO_SRGB;
    let r = delinearized(m[0][0] * x + m[0][1] * y + m[0][2] * z);
    let g = delinearized(m[1][0] * x + m[1][1] * y + m[1][2] * z);
    let b = delinearized(m[2][0] * x + m[2][1] * y + m[2][2] * z);
    0xFF00_0000 | r << 16 | g << 8 | b
}

fn delinearized(component: f64) -> u32 {
    let normalized = component / 100.0;
    let v = if normalized <= 0.0031308 { normalized * 12.92 } else { 1.055 * normalized.powf(1.0 / 2.4) - 0.055 };
    // (Kotlin's Math.round: half up)
    ((v * 255.0 + 0.5).floor() as i64).clamp(0, 255) as u32
}

// ---- Compose's float CAM16 (internal colorUtil's Cam.fromInt in its default Frame) ------------------------------

const XYZ_TO_CAM16RGB: [[f32; 3]; 3] =
    [[0.401288, 0.650173, -0.051461], [-0.250268, 1.204414, 0.045854], [-0.002079, 0.048952, 0.953127]];
const WHITE_POINT_D65: [f32; 3] = [95.047, 100.0, 108.883];
const SRGB_TO_XYZ: [[f64; 3]; 3] =
    [[0.41233895, 0.35762064, 0.18051042], [0.2126, 0.7152, 0.0722], [0.01932141, 0.11916382, 0.95034478]];
const XYZ_TO_SRGB: [[f64; 3]; 3] = [
    [3.2413774792388685, -1.5376652402851851, -0.49885366846268053],
    [-0.9691452513005321, 1.8758853451067872, 0.04156585616912061],
    [0.05562093689691305, -0.20395524564742123, 1.0571799111220335],
];

/// The viewing conditions of sRGB's standard (Compose's internal `Frame.Default`).
struct Frame {
    n: f32,
    aw: f32,
    nbb: f32,
    ncb: f32,
    c: f32,
    nc: f32,
    rgb_d: [f32; 3],
    fl: f32,
    z: f32,
}

fn y_from_lstar(lstar: f64) -> f64 {
    if lstar > 8.0 { ((lstar + 16.0) / 116.0).powf(3.0) * 100.0 } else { lstar / (24389.0 / 27.0) * 100.0 }
}

fn lerp(start: f32, stop: f32, amount: f32) -> f32 {
    (1.0 - amount) * start + amount * stop
}

/// Compose's `Frame.make(WHITE_POINT_D65, (200 / π · Y(50) / 100), 50, 2, false)`.
fn default_frame() -> Frame {
    let whitepoint = WHITE_POINT_D65;
    let adapting_luminance = (200.0f64 / std::f64::consts::PI * y_from_lstar(50.0) / 100.0) as f32;
    let background_lstar = 50.0f32;
    let surround = 2.0f32;
    let m = XYZ_TO_CAM16RGB;
    let r_w = whitepoint[0] * m[0][0] + whitepoint[1] * m[0][1] + whitepoint[2] * m[0][2];
    let g_w = whitepoint[0] * m[1][0] + whitepoint[1] * m[1][1] + whitepoint[2] * m[1][2];
    let b_w = whitepoint[0] * m[2][0] + whitepoint[1] * m[2][1] + whitepoint[2] * m[2][2];
    let f = 0.8 + (surround / 10.0);
    let c = if f >= 0.9 { lerp(0.59, 0.69, (f - 0.9) * 10.0) } else { lerp(0.525, 0.59, (f - 0.8) * 10.0) };
    let d = f * (1.0 - ((1.0 / 3.6) * (((-adapting_luminance - 42.0) / 92.0) as f64).exp() as f32));
    let d = d.clamp(0.0, 1.0);
    let nc = f;
    let rgb_d = [d * (100.0 / r_w) + 1.0 - d, d * (100.0 / g_w) + 1.0 - d, d * (100.0 / b_w) + 1.0 - d];
    let k = 1.0 / (5.0 * adapting_luminance + 1.0);
    let k4 = k * k * k * k;
    let k4f = 1.0 - k4;
    let fl = (k4 * adapting_luminance) + (0.1 * k4f * k4f * (5.0 * adapting_luminance as f64).cbrt() as f32);
    let n = y_from_lstar(background_lstar as f64) as f32 / whitepoint[1];
    let z = 1.48 + n.sqrt();
    let nbb = 0.725 / n.powf(0.2);
    let factors = [
        (fl * rgb_d[0] * r_w / 100.0).powf(0.42),
        (fl * rgb_d[1] * g_w / 100.0).powf(0.42),
        (fl * rgb_d[2] * b_w / 100.0).powf(0.42),
    ];
    let rgb_a = factors.map(|f| (400.0 * f) / (f + 27.13));
    let aw = ((2.0 * rgb_a[0]) + rgb_a[1] + (0.05 * rgb_a[2])) * nbb;
    Frame { n, aw, nbb, ncb: nbb, c, nc, rgb_d, fl, z }
}

fn linearized(component: u32) -> f32 {
    let normalized = component as f32 / 255.0;
    if normalized <= 0.04045 { normalized / 12.92 * 100.0 } else { ((normalized + 0.055) / 1.055).powf(2.4) * 100.0 }
}

/// A color's CAM16 hue and chroma in sRGB's frame, in Compose's floats (`Cam.fromInt`).
fn cam_hue_chroma(argb: u32) -> (f32, f32) {
    let frame = default_frame();
    let (r, g, b) = (linearized(argb >> 16 & 0xFF), linearized(argb >> 8 & 0xFF), linearized(argb & 0xFF));
    let s = SRGB_TO_XYZ;
    let xyz = [
        (r as f64 * s[0][0] + g as f64 * s[0][1] + b as f64 * s[0][2]) as f32,
        (r as f64 * s[1][0] + g as f64 * s[1][1] + b as f64 * s[1][2]) as f32,
        (r as f64 * s[2][0] + g as f64 * s[2][1] + b as f64 * s[2][2]) as f32,
    ];
    let m = XYZ_TO_CAM16RGB;
    let r_t = xyz[0] * m[0][0] + xyz[1] * m[0][1] + xyz[2] * m[0][2];
    let g_t = xyz[0] * m[1][0] + xyz[1] * m[1][1] + xyz[2] * m[1][2];
    let b_t = xyz[0] * m[2][0] + xyz[1] * m[2][1] + xyz[2] * m[2][2];
    let (r_d, g_d, b_d) = (frame.rgb_d[0] * r_t, frame.rgb_d[1] * g_t, frame.rgb_d[2] * b_t);
    let adapted = |v: f32| {
        let af = (frame.fl * v.abs() / 100.0).powf(0.42);
        v.signum_or_zero() * 400.0 * af / (af + 27.13)
    };
    let (r_a, g_a, b_a) = (adapted(r_d), adapted(g_d), adapted(b_d));
    let a = (11.0 * r_a + -12.0 * g_a + b_a) / 11.0;
    let b = (r_a + g_a - 2.0 * b_a) / 9.0;
    let u = (20.0 * r_a + 20.0 * g_a + 21.0 * b_a) / 20.0;
    let p2 = (40.0 * r_a + 20.0 * g_a + b_a) / 20.0;
    let atan_degrees = b.atan2(a) * 180.0 / std::f32::consts::PI;
    let hue = if atan_degrees < 0.0 {
        atan_degrees + 360.0
    } else if atan_degrees >= 360.0 {
        atan_degrees - 360.0
    } else {
        atan_degrees
    };
    let ac = p2 * frame.nbb;
    let j = 100.0 * (ac / frame.aw).powf(frame.c * frame.z);
    let hue_prime = if (hue as f64) < 20.14 { hue + 360.0 } else { hue };
    let e_hue = 0.25 * ((hue_prime * std::f32::consts::PI / 180.0 + 2.0).cos() + 3.8);
    let p1 = 50000.0 / 13.0 * e_hue * frame.nc * frame.ncb;
    let t = p1 * (a * a + b * b).sqrt() / (u + 0.305);
    let alpha = t.powf(0.9) * (1.64 - 0.29f32.powf(frame.n)).powf(0.73);
    let chroma = alpha * (j / 100.0).sqrt();
    (hue, chroma)
}

/// Kotlin's `sign` of a float: -1, 0 or 1 (Rust's `signum` gives 1 for +0).
trait SignumOrZero {
    fn signum_or_zero(self) -> f32;
}

impl SignumOrZero for f32 {
    fn signum_or_zero(self) -> f32 {
        if self == 0.0 || self.is_nan() { self } else { self.signum() }
    }
}
