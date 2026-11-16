//! Colors (Compose's `androidx.compose.ui.graphics.Color`): 64 bits packed as Compose packs them, the color
//! spaces they are in, how they convert, blend, measure and animate. The arithmetic is Compose's, operation for
//! operation, so the bits are its own.

use crate::colorspace::{ColorSpace, ColorSpaces, MIN_ID, Rgb, same_space};
use crate::geometry::lerp;
use kompoz_runtime::named;
use std::fmt;

/// Compose's floatToHalf (its Float16's), bit for bit.
pub const fn float_to_half(f: f32) -> u16 {
    let bits = f.to_bits();
    let s = bits >> 31;
    let mut e = (bits >> 23 & 0xff) as i32;
    let mut m = bits & 0x7fffff;
    let mut out_e = 0u32;
    let mut out_m = 0u32;
    if e == 0xff {
        // infinite or NaN
        out_e = 0x1f;
        out_m = if m != 0 { 0x200 } else { 0 };
    } else {
        e = e - 127 + 15;
        if e >= 0x1f {
            // overflow (as Compose writes it)
            out_e = 0x31;
        } else if e <= 0 {
            // underflow
            if e >= -10 {
                // a normal float under a half's smallest normal: a subnormal half
                m = (m | 0x800000) >> (1 - e);
                if m & 0x1000 != 0 {
                    m += 0x2000;
                }
                out_m = m >> 13;
            }
        } else {
            out_e = e as u32;
            out_m = m >> 13;
            if m & 0x1000 != 0 {
                // (a half up)
                return (((out_e << 10 | out_m) + 1) | s << 15) as u16;
            }
        }
    }
    (s << 15 | out_e << 10 | out_m) as u16
}

/// Compose's halfToFloat, bit for bit.
pub const fn half_to_float(h: u16) -> f32 {
    let bits = h as u32;
    let s = bits & 0x8000;
    let e = bits >> 10 & 0x1f;
    let m = bits & 0x3ff;
    let mut out_e = 0u32;
    let mut out_m = 0u32;
    if e == 0 {
        // subnormal or 0
        if m != 0 {
            let o = f32::from_bits((126 << 23) + m) - f32::from_bits(126 << 23);
            return if s == 0 { o } else { -o };
        }
    } else {
        out_m = m << 13;
        if e == 0x1f {
            // infinite or NaN
            out_e = 0xff;
            if out_m != 0 {
                out_m |= 0x400000;
            }
        } else {
            out_e = e + 127 - 15;
        }
    }
    f32::from_bits(s << 16 | out_e << 23 | out_m)
}

/// A channel 0..1 in 8 bits, as Compose packs an sRGB color's (NaN as 0).
const fn byte_of(v: f32) -> u64 {
    if v.is_nan() { 0 } else { (v.clamp(0.0, 1.0) * 255.0 + 0.5) as i32 as u64 }
}

/// An alpha 0..1 in 10 bits, as Compose packs another space's (NaN as 0).
const fn alpha10_of(alpha: f32) -> u64 {
    if alpha.is_nan() { 0 } else { (alpha.clamp(0.0, 1.0) * 1023.0 + 0.5) as i32 as u64 }
}

/// C++'s std::min and std::max, as Compose's coerce does them (a NaN on one side decides).
fn min_of(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}
fn max_of(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// A color (Compose's Color): 64 bits, packed as Compose packs them. One in sRGB keeps 8 bits a channel, 0..1
/// (its ARGB in the upper 32 bits); one in another color space keeps its components as half floats (each within
/// the space's range), its alpha in 10 bits and the space's id ([`ColorSpaces`]). [`red`](Color::red),
/// [`green`](Color::green), [`blue`](Color::blue) are the components in its color space (an Lab or Oklab one's
/// L, a, b), [`alpha`](Color::alpha) its alpha.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    value: u64,
}

/// Compose's `Color(…)`, its overloads by their arguments: `Color(0xFF6200EE)` (a `Long`: ARGB in its low 32 bits),
/// `Color(argb: Int)`, `Color((red, green, blue))` and `Color((red, green, blue, alpha))` of `Int`s (0..255, masked)
/// or of `Float`s (0..1, kept within, in sRGB), `Color((red, green, blue, alpha, color_space))`; a `u32` is ARGB, a
/// `u64` the packed bits (Compose's `Color(value: ULong)`).
pub fn Color(args: impl ColorArgs) -> Color {
    args.color()
}

/// What [`Color()`](fn@Color) is given: Kotlin's arguments, one as itself, several as a tuple, each overload by its
/// types.
pub trait ColorArgs {
    fn color(self) -> Color;
}
/// Kotlin: `Color(color: Int)`
impl ColorArgs for i32 {
    fn color(self) -> Color {
        Color::from_argb(self as u32)
    }
}
/// Kotlin: `Color(color: Long)`
impl ColorArgs for i64 {
    fn color(self) -> Color {
        Color::from_argb(self as u32)
    }
}
/// 0xAARRGGBB (Rust's, as [`Color::from_argb`]).
impl ColorArgs for u32 {
    fn color(self) -> Color {
        Color::from_argb(self)
    }
}
/// Kotlin: `Color(value: ULong)`
impl ColorArgs for u64 {
    fn color(self) -> Color {
        Color::from_value(self)
    }
}
/// Kotlin: `Color(red: Int, green: Int, blue: Int)`
impl ColorArgs for (i32, i32, i32) {
    fn color(self) -> Color {
        (self.0, self.1, self.2, 0xFF).color()
    }
}
/// Kotlin: `Color(red: Int, green: Int, blue: Int, alpha: Int)`
impl ColorArgs for (i32, i32, i32, i32) {
    fn color(self) -> Color {
        let (r, g, b, a) = self;
        Color::from_argb(((a & 0xFF) << 24 | (r & 0xFF) << 16 | (g & 0xFF) << 8 | (b & 0xFF)) as u32)
    }
}
/// Kotlin: `Color(red: Float, green: Float, blue: Float)`
impl ColorArgs for (f32, f32, f32) {
    fn color(self) -> Color {
        Color::new(self.0, self.1, self.2, 1.0)
    }
}
/// Kotlin: `Color(red: Float, green: Float, blue: Float, alpha: Float)`
impl ColorArgs for (f32, f32, f32, f32) {
    fn color(self) -> Color {
        Color::new(self.0, self.1, self.2, self.3)
    }
}
/// Kotlin: `Color(red: Float, green: Float, blue: Float, alpha: Float, colorSpace: ColorSpace)`
impl<S: ColorSpace> ColorArgs for (f32, f32, f32, f32, &S) {
    fn color(self) -> Color {
        Color::new_in(self.0, self.1, self.2, self.3, self.4)
    }
}
/// Kotlin: `Color(red: Float, green: Float, blue: Float, alpha: Float, colorSpace: ColorSpace)`
impl ColorArgs for (f32, f32, f32, f32, &dyn ColorSpace) {
    fn color(self) -> Color {
        Color::new_in(self.0, self.1, self.2, self.3, self.4)
    }
}

impl Color {
    pub const Transparent: Color = Color::from_argb(0x00000000);
    pub const Black: Color = Color::from_argb(0xFF000000);
    pub const DarkGray: Color = Color::from_argb(0xFF444444);
    pub const Gray: Color = Color::from_argb(0xFF888888);
    pub const LightGray: Color = Color::from_argb(0xFFCCCCCC);
    pub const White: Color = Color::from_argb(0xFFFFFFFF);
    pub const Red: Color = Color::from_argb(0xFFFF0000);
    pub const Green: Color = Color::from_argb(0xFF00FF00);
    pub const Blue: Color = Color::from_argb(0xFF0000FF);
    pub const Yellow: Color = Color::from_argb(0xFFFFFF00);
    pub const Cyan: Color = Color::from_argb(0xFF00FFFF);
    pub const Magenta: Color = Color::from_argb(0xFFFF00FF);
    /// No color: 0x10, in ColorSpaces::Unspecified (none of Transparent's).
    pub const Unspecified: Color = Color::from_value(0x10);

    /// The packed bits (Compose's Color(value)).
    pub const fn from_value(value: u64) -> Color {
        Color { value }
    }

    /// In sRGB, each channel kept within 0..1 (Compose's Color(red, green, blue, alpha)).
    pub const fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Color {
        Color::from_value((byte_of(alpha) << 24 | byte_of(red) << 16 | byte_of(green) << 8 | byte_of(blue)) << 32)
    }

    /// In `color_space`, each component kept within its range (Compose's Color(red, green, blue, alpha,
    /// colorSpace)). Panics, as Compose throws, for a space of other than three components, or one of none of
    /// ColorSpaces' ids (unless it is sRGB).
    pub fn new_in(red: f32, green: f32, blue: f32, alpha: f32, color_space: &dyn ColorSpace) -> Color {
        if color_space.is_srgb() {
            return Color::new(red, green, blue, alpha);
        }
        if color_space.component_count() != 3 {
            panic!("Color only works with ColorSpaces with 3 components");
        }
        let id = color_space.id();
        if id == MIN_ID {
            panic!("Unknown color space, please use a color space in ColorSpaces");
        }
        let r = float_to_half(red.clamp(color_space.get_min_value(0), color_space.get_max_value(0))) as u64;
        let g = float_to_half(green.clamp(color_space.get_min_value(1), color_space.get_max_value(1))) as u64;
        let b = float_to_half(blue.clamp(color_space.get_min_value(2), color_space.get_max_value(2))) as u64;
        let a = alpha10_of(alpha);
        Color::from_value(r << 48 | g << 32 | b << 16 | (a & 0x3ff) << 6 | (id as u64 & 0x3f))
    }

    /// 0xAARRGGBB, in sRGB (Compose's Color(color: Int)).
    pub const fn from_argb(argb: u32) -> Color {
        Color::from_value((argb as u64) << 32)
    }

    /// 0xRRGGBB, opaque, in sRGB.
    pub const fn hex(rgb: u32) -> Color {
        Color::from_argb(0xFF000000 | (rgb & 0xFFFFFF))
    }

    /// From hue (0..360), saturation and value (0..1), in sRGB (Compose's Color.hsv). Panics outside those ranges.
    pub fn hsv(hue: f32, saturation: f32, value: f32, alpha: f32) -> Color {
        Color::hsv_in(hue, saturation, value, alpha, &ColorSpaces::Srgb)
    }

    /// The same in an RGB color space.
    pub fn hsv_in(hue: f32, saturation: f32, value: f32, alpha: f32, color_space: &Rgb) -> Color {
        if !((0.0..=360.0).contains(&hue) && (0.0..=1.0).contains(&saturation) && (0.0..=1.0).contains(&value)) {
            panic!("HSV ({hue}, {saturation}, {value}) must be in range (0..360, 0..1, 0..1)");
        }
        let component = |n: i32| {
            let k = (n as f32 + hue / 60.0) % 6.0;
            value - (value * saturation * 0f32.max(k.min(4.0 - k).min(1.0)))
        };
        Color::new_in(component(5), component(3), component(1), alpha, color_space)
    }

    /// From hue (0..360), saturation and lightness (0..1), in sRGB (Compose's Color.hsl). Panics outside those
    /// ranges.
    pub fn hsl(hue: f32, saturation: f32, lightness: f32, alpha: f32) -> Color {
        Color::hsl_in(hue, saturation, lightness, alpha, &ColorSpaces::Srgb)
    }

    /// The same in an RGB color space.
    pub fn hsl_in(hue: f32, saturation: f32, lightness: f32, alpha: f32, color_space: &Rgb) -> Color {
        if !((0.0..=360.0).contains(&hue) && (0.0..=1.0).contains(&saturation) && (0.0..=1.0).contains(&lightness)) {
            panic!("HSL ({hue}, {saturation}, {lightness}) must be in range (0..360, 0..1, 0..1)");
        }
        let component = |n: i32| {
            let k = (n as f32 + hue / 30.0) % 12.0;
            let a = saturation * lightness.min(1.0 - lightness);
            lightness - a * (-1f32).max((k - 3.0).min(9.0 - k).min(1.0))
        };
        Color::new_in(component(0), component(8), component(4), alpha, color_space)
    }

    /// The packed bits.
    pub const fn value(self) -> u64 {
        self.value
    }

    /// In sRGB (its id, 0).
    pub const fn is_srgb(self) -> bool {
        self.value & 0x3f == 0
    }

    pub const fn red(self) -> f32 {
        if self.is_srgb() { (self.value >> 48 & 0xff) as f32 / 255.0 } else { half_to_float((self.value >> 48) as u16) }
    }
    pub const fn green(self) -> f32 {
        if self.is_srgb() { (self.value >> 40 & 0xff) as f32 / 255.0 } else { half_to_float((self.value >> 32) as u16) }
    }
    pub const fn blue(self) -> f32 {
        if self.is_srgb() { (self.value >> 32 & 0xff) as f32 / 255.0 } else { half_to_float((self.value >> 16) as u16) }
    }
    pub const fn alpha(self) -> f32 {
        if self.is_srgb() {
            (self.value >> 56 & 0xff) as f32 / 255.0
        } else {
            (self.value >> 6 & 0x3ff) as f32 / 1023.0
        }
    }

    pub fn color_space(self) -> &'static dyn ColorSpace {
        ColorSpaces::get_color_space((self.value & 0x3f) as i32)
    }

    /// The same color in `color_space` (Compose's convert: through the spaces' connector, perceptually).
    pub fn convert(self, color_space: &dyn ColorSpace) -> Color {
        let source = self.color_space();
        if same_space(source, color_space) {
            return self; // (the identity connector's)
        }
        let v = source.connect(color_space).transform(self.red(), self.green(), self.blue());
        Color::new_in(v[0], v[1], v[2], self.alpha(), color_space)
    }

    /// This over `background`, in the background's color space (Compose's compositeOver).
    pub fn composite_over(self, background: Color) -> Color {
        let space = background.color_space();
        let fg = self.convert(space);
        let (bg_a, fg_a) = (background.alpha(), fg.alpha());
        let a = fg_a + (bg_a * (1.0 - fg_a));
        let component =
            |fg_c: f32, bg_c: f32| if a == 0.0 { 0.0 } else { ((fg_c * fg_a) + ((bg_c * bg_a) * (1.0 - fg_a))) / a };
        unchecked_color(
            component(fg.red(), background.red()),
            component(fg.green(), background.green()),
            component(fg.blue(), background.blue()),
            a,
            space,
        )
    }

    /// Its relative luminance, 0..1 (Compose's luminance: an RGB color space's, through its EOTF). Panics for a
    /// color of another model, as Compose throws.
    pub fn luminance(self) -> f32 {
        let space = self.color_space();
        let Some(rgb) = space.as_rgb() else {
            panic!(
                "The specified color must be encoded in an RGB color space. The supplied color space is {}",
                space.model()
            );
        };
        let (r, g, b) = (rgb.eotf(self.red() as f64), rgb.eotf(self.green() as f64), rgb.eotf(self.blue() as f64));
        (((0.2126 * r) + (0.7152 * g) + (0.0722 * b)) as f32).clamp(0.0, 1.0)
    }

    /// In sRGB, 0xAARRGGBB (Compose's toArgb).
    pub fn to_argb(self) -> u32 {
        (self.convert(&ColorSpaces::Srgb).value >> 32) as u32
    }

    /// Not [`Color::Unspecified`].
    pub const fn is_specified(self) -> bool {
        self.value != 0x10
    }
    pub const fn is_unspecified(self) -> bool {
        self.value == 0x10
    }
    /// This, or `block`'s where this is unspecified (Compose's takeOrElse).
    pub fn take_or_else(self, block: impl FnOnce() -> Color) -> Color {
        if self.is_specified() { self } else { block() }
    }

    /// The color in sRGB, as it is drawn (to an sRGB surface, as Compose's to Android's), 8 bits a channel.
    pub fn in_srgb(self) -> Color {
        if self.is_srgb() { self } else { self.convert(&ColorSpaces::Srgb) }
    }

    /// Its components in sRGB, red, green, blue and alpha, not rounded to 8 bits (as Skia draws a paint's color):
    /// another space's in extended sRGB (past 0..1 where it is past sRGB).
    pub fn srgb_components(self) -> [f32; 4] {
        if self.is_srgb() {
            return [self.red(), self.green(), self.blue(), self.alpha()];
        }
        let v = self.color_space().connect(&ColorSpaces::ExtendedSrgb).transform(self.red(), self.green(), self.blue());
        [v[0], v[1], v[2], self.alpha()]
    }
}

#[named]
impl Color {
    /// Other components, in its color space, each the color's own unless given (Compose's copy: `c.copy(alpha =
    /// 0.5)` in a `#[kompozable]` body).
    pub fn copy(
        &self,
        #[default(self.alpha())] alpha: f32,
        #[default(self.red())] red: f32,
        #[default(self.green())] green: f32,
        #[default(self.blue())] blue: f32,
    ) -> Color {
        Color::new_in(red, green, blue, alpha, self.color_space())
    }
}

/// A color given when it is asked for (Compose's ColorProducer): what a composable draws in, read as it draws, so
/// an animated color redraws it without composing it again. Equal only to itself (a clone of it).
#[derive(Clone)]
pub struct ColorProducer(std::rc::Rc<dyn Fn() -> Color>);

impl ColorProducer {
    pub fn new(produce: impl Fn() -> Color + 'static) -> ColorProducer {
        ColorProducer(std::rc::Rc::new(produce))
    }
    /// The color now (Compose's `invoke`): unspecified for none.
    pub fn get(&self) -> Color {
        (self.0)()
    }
}

impl PartialEq for ColorProducer {
    fn eq(&self, other: &ColorProducer) -> bool {
        std::rc::Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for ColorProducer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ColorProducer({:p})", std::rc::Rc::as_ptr(&self.0) as *const ())
    }
}

/// Opaque black (Compose's Color has no default).
impl Default for Color {
    fn default() -> Color {
        Color::Black
    }
}

/// Compose's toString: "Color(red, green, blue, alpha, the color space's name)".
impl fmt::Debug for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Color({:?}, {:?}, {:?}, {:?}, {})",
            self.red(),
            self.green(),
            self.blue(),
            self.alpha(),
            self.color_space().name()
        )
    }
}

/// Compose's UncheckedColor: packed as given, nothing kept within a range (an sRGB one's channels only as Kotlin's
/// toInt saturates them).
fn unchecked_color(red: f32, green: f32, blue: f32, alpha: f32, color_space: &dyn ColorSpace) -> Color {
    if color_space.is_srgb() {
        // (Kotlin's toInt and Rust's `as`: NaN 0, saturated.)
        let byte = |v: f32| (v * 255.0 + 0.5) as i32 as u32;
        let argb = byte(alpha) << 24 | byte(red) << 16 | byte(green) << 8 | byte(blue);
        return Color::from_argb(argb);
    }
    let (r, g, b) = (float_to_half(red) as u64, float_to_half(green) as u64, float_to_half(blue) as u64);
    let a = (max_of(0.0, min_of(alpha, 1.0)) * 1023.0 + 0.5) as i32 as u64;
    Color::from_value(r << 48 | g << 32 | b << 16 | (a & 0x3ff) << 6 | (color_space.id() as u64 & 0x3f))
}

/// Between two colors, in Oklab, in `stop`'s color space (Compose's lerp: `fraction` kept within 0..1, so an
/// easing's overshoot stays within the colors).
pub fn lerp_color(start: Color, stop: Color, fraction: f32) -> Color {
    let oklab = &ColorSpaces::Oklab;
    let (s, e) = (start.convert(oklab), stop.convert(oklab));
    let t = fraction.clamp(0.0, 1.0);
    let interpolated = unchecked_color(
        lerp(s.red(), e.red(), t),
        lerp(s.green(), e.green(), t),
        lerp(s.blue(), e.blue(), t),
        lerp(s.alpha(), e.alpha(), t),
        oklab,
    );
    interpolated.convert(stop.color_space())
}

/// `fraction` of the way from `from` to `to` as Compose animates a color (its ColorVectorConverter): in Oklab,
/// each component as far along (past 0..1 too, as a spring overshoots it), kept within Oklab's range and alpha
/// within 0..1, in `to`'s color space.
pub fn animated_color(from: Color, to: Color, fraction: f32) -> Color {
    let oklab = &ColorSpaces::Oklab;
    let (s, e) = (from.convert(oklab), to.convert(oklab));
    // (Color keeps L within 0..1 and a, b within -0.5..0.5, as the converter coerces them.)
    Color::new_in(
        lerp(s.red(), e.red(), fraction),
        lerp(s.green(), e.green(), fraction),
        lerp(s.blue(), e.blue(), fraction),
        lerp(s.alpha(), e.alpha(), fraction),
        oklab,
    )
    .convert(to.color_space())
}
