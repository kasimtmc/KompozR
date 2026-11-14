//! Colors, color spaces, HCT, palettes, dynamic schemes and a picture's colors: Compose's ColorSpaceTest's and
//! ColorTest's numbers (as Kompoz's tests have them), Material Color Utilities' and Android's.

// (Compose's numbers as its tests write them.)
#![allow(clippy::excessive_precision)]

use kompoz_graphics::colorspace::*;
use kompoz_graphics::material_color::contrast::Contrast;
use kompoz_graphics::material_color::dislike::DislikeAnalyzer;
use kompoz_graphics::material_color::dynamiccolor::*;
use kompoz_graphics::material_color::hct::{Cam16, Hct, ViewingConditions, cam_to_color};
use kompoz_graphics::material_color::palettes::{CorePalette, DynamicTonalPalette, TonalPalette, palette_tone};
use kompoz_graphics::material_color::quantize::QuantizerCelebi;
use kompoz_graphics::material_color::scheme::*;
use kompoz_graphics::material_color::score::Score;
use kompoz_graphics::material_color::temperature::TemperatureCache;
use kompoz_graphics::material_color::wallpaper::{GOOGLE_BLUE, WallpaperColors, seed_colors};
use kompoz_graphics::{Color, animated_color, float_to_half, half_to_float, lerp, lerp_color};
use kompoz_runtime::kompozable_block;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() < tolerance
}

fn near64(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() < tolerance
}

/// Compose's assertArrayEquals: each within `eps` (or the same float).
fn near_all<const N: usize>(a: [f32; N], b: [f32; N], eps: f32) -> bool {
    a.iter().zip(&b).all(|(&x, &y)| x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()) || (x - y).abs() <= eps)
}

/// Whether `f` panics (where Compose throws).
fn panics(f: impl FnOnce()) -> bool {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let panicked = catch_unwind(AssertUnwindSafe(f)).is_err();
    std::panic::set_hook(hook);
    panicked
}

fn dyn_space(space: &dyn ColorSpace) -> &dyn ColorSpace {
    space
}

// ---- Color spaces: Compose's ColorSpaceTest's numbers, and more ---------------------------------------------------

// Compose's ColorSpaceTest's: sRGB's matrices (column-major), primaries and white point.
const SRGB_TO_XYZ: [f32; 9] =
    [0.412391, 0.212639, 0.019331, 0.357584, 0.715169, 0.119195, 0.180481, 0.072192, 0.950532];
const XYZ_TO_SRGB: [f32; 9] =
    [3.240970, -0.969244, 0.055630, -1.537383, 1.875968, -0.203977, -0.498611, 0.041555, 1.056971];
const SRGB_TO_XYZ_D50: [f32; 9] =
    [0.4360747, 0.2225045, 0.0139322, 0.3850649, 0.7168786, 0.0971045, 0.1430804, 0.0606169, 0.7141733];
const SRGB_PRIMARIES_XYY: [f32; 6] = [0.640, 0.330, 0.300, 0.600, 0.150, 0.060];
const SRGB_PRIMARIES_XYZ: [f32; 9] =
    [1.939394, 1.000000, 0.090909, 0.500000, 1.000000, 0.166667, 2.500000, 1.000000, 13.166667];
const SRGB_WHITE_POINT_XYY: WhitePoint = WhitePoint::new(0.3127, 0.3290);
const SRGB_WHITE_POINT_XYZ: WhitePoint = WhitePoint::from_xyz(0.950456, 1.000, 1.089058);

fn identity(x: f64) -> f64 {
    x
}

fn srgb_parameters() -> TransferParameters {
    TransferParameters::new(2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045, 0.0, 0.0)
}

#[test]
fn color_spaces_have_composes_ids_names_and_models() {
    let named: [&dyn ColorSpace; 20] = [
        &ColorSpaces::Srgb,
        &ColorSpaces::LinearSrgb,
        &ColorSpaces::ExtendedSrgb,
        &ColorSpaces::LinearExtendedSrgb,
        &ColorSpaces::Bt709,
        &ColorSpaces::Bt2020,
        &ColorSpaces::DciP3,
        &ColorSpaces::DisplayP3,
        &ColorSpaces::Ntsc1953,
        &ColorSpaces::SmpteC,
        &ColorSpaces::AdobeRgb,
        &ColorSpaces::ProPhotoRgb,
        &ColorSpaces::Aces,
        &ColorSpaces::Acescg,
        &ColorSpaces::CieXyz,
        &ColorSpaces::CieLab,
        &ColorSpaces::Unspecified,
        &ColorSpaces::Bt2020Hlg,
        &ColorSpaces::Bt2020Pq,
        &ColorSpaces::Oklab,
    ];
    for id in 0..20 {
        // The ids colors keep: these cannot change.
        let space = named[id as usize];
        assert!(same_space(ColorSpaces::get_color_space(id), space) && space.id() == id);
        assert!(space.component_count() == 3 && !space.name().is_empty());
        let model = if id == 14 {
            ColorModel::Xyz
        } else if id == 15 || id == 19 {
            ColorModel::Lab
        } else {
            ColorModel::Rgb
        };
        assert_eq!(space.model(), model);
    }
    assert_eq!((MIN_ID, MAX_ID), (-1, 63));
    assert!(ColorSpaces::Srgb.name() == "sRGB IEC61966-2.1" && ColorSpaces::Unspecified.name() == "None");
    assert_eq!(ColorSpaces::CieLab.name(), "Generic L*a*b*");
    assert_eq!(ColorSpaces::Srgb.to_string(), "sRGB IEC61966-2.1 (id=0, model=Rgb)");
    assert_eq!(ColorSpaces::Oklab.to_string(), "Oklab (id=19, model=Lab)");
    assert!(panics(|| {
        ColorSpaces::get_color_space(20);
    }));
    assert!(ColorModel::Cmyk.component_count() == 4 && ColorModel::Lab.component_count() == 3);
    assert_eq!(ColorModel::Xyz.to_string(), "Xyz");
    assert_eq!(RenderIntent::Absolute.to_string(), "Absolute");
    assert_eq!(Adaptation::Ciecat02.to_string(), "Ciecat02");
}

#[test]
fn color_spaces_ranges_wide_gamuts_and_srgb_as_composes() {
    assert!(ColorSpaces::Srgb.get_min_value(0) == 0.0 && ColorSpaces::Srgb.get_max_value(2) == 1.0);
    let lab = &ColorSpaces::CieLab;
    assert!(lab.get_min_value(0) == 0.0 && lab.get_min_value(1) == -128.0 && lab.get_max_value(0) == 100.0);
    assert_eq!(lab.get_max_value(2), 128.0);
    assert!(ColorSpaces::CieXyz.get_min_value(1) == -2.0 && ColorSpaces::CieXyz.get_max_value(1) == 2.0);
    let oklab = &ColorSpaces::Oklab;
    assert!(oklab.get_min_value(0) == 0.0 && oklab.get_min_value(1) == -0.5 && oklab.get_max_value(0) == 1.0);
    assert_eq!(oklab.get_max_value(2), 0.5);
    assert!(
        ColorSpaces::ExtendedSrgb.get_min_value(0) == -0.799 && ColorSpaces::ExtendedSrgb.get_max_value(0) == 2.399
    );
    assert!(ColorSpaces::LinearExtendedSrgb.get_min_value(0) == -0.5 && ColorSpaces::Aces.get_max_value(0) == 65504.0);
    // Wide: over 90% of NTSC 1953's area holding sRGB's, or a range past 0..1 both ways (Lab, XYZ, Oklab: always).
    // As Compose tells holding, NTSC 1953 and ProPhoto RGB do not (sRGB's blue past their blue's corner): not wide.
    let wide = [
        false, false, true, true, false, true, true, true, false, false, true, false, true, true, true, true, false,
        true, true, true,
    ];
    for id in 0..20 {
        assert_eq!(ColorSpaces::get_color_space(id).is_wide_gamut(), wide[id as usize], "{id}");
        assert_eq!(ColorSpaces::get_color_space(id).is_srgb(), id == 0, "{id}");
    }
    // Compose decides by the primaries, white point, range and transfer functions, not the id.
    let almost = Rgb::new("Almost sRGB", &SRGB_TO_XYZ, |x| x.powf(1.0 / 2.2), |x| x.powf(2.2));
    assert!(!almost.is_srgb());
    let same = Rgb::from_primaries_with_parameters("Mine", &SRGB_PRIMARIES_XYY, Illuminant::D65, srgb_parameters());
    assert!(same.is_srgb() && same.id() == MIN_ID && !same.is_wide_gamut());
    let wider = Rgb::from_primaries("Wider", &SRGB_PRIMARIES_XYY, Illuminant::D65, identity, identity, -1.0, 2.0);
    assert!(wider.is_wide_gamut() && !wider.is_srgb());
}

#[test]
fn rgb_color_spaces_make_composes_matrices_primaries_and_white_points() {
    let from_matrix = Rgb::new("Test", &SRGB_TO_XYZ, identity, identity);
    assert!(
        near_all(from_matrix.transform(), SRGB_TO_XYZ, 1e-5)
            && near_all(from_matrix.inverse_transform(), XYZ_TO_SRGB, 1e-5)
    );
    assert!(near_all(from_matrix.primaries(), SRGB_PRIMARIES_XYY, 1e-5));
    let wp = from_matrix.white_point();
    assert!(near(wp.x, SRGB_WHITE_POINT_XYY.x, 1e-5) && near(wp.y, SRGB_WHITE_POINT_XYY.y, 1e-5));
    let from_xyy = Rgb::from_primaries("Test", &SRGB_PRIMARIES_XYY, SRGB_WHITE_POINT_XYY, identity, identity, 0.0, 1.0);
    assert!(
        near_all(from_xyy.transform(), SRGB_TO_XYZ, 1e-5) && near_all(from_xyy.inverse_transform(), XYZ_TO_SRGB, 1e-5)
    );
    let from_xyz = Rgb::from_primaries("Test", &SRGB_PRIMARIES_XYZ, SRGB_WHITE_POINT_XYZ, identity, identity, 0.0, 1.0);
    assert!(near_all(from_xyz.primaries(), SRGB_PRIMARIES_XYY, 1e-3));
    assert!(near(from_xyz.white_point().x, 0.3127, 1e-3) && near(from_xyz.white_point().y, 0.3290, 1e-3));
    assert!(
        near_all(from_xyz.transform(), SRGB_TO_XYZ, 1e-5) && near_all(from_xyz.inverse_transform(), XYZ_TO_SRGB, 1e-5)
    );
    assert!(
        ColorSpaces::Srgb.white_point() == Illuminant::D65
            && near_all(ColorSpaces::Srgb.primaries(), SRGB_PRIMARIES_XYY, 1e-9)
    );
    assert!(ColorSpaces::DciP3.white_point() == WhitePoint::new(0.314, 0.351));
    assert!(ColorSpaces::ProPhotoRgb.white_point() == Illuminant::D50);
    // What Compose rejects.
    let zeros = [0.0f32; 6];
    let seven = [0.0f32; 7];
    assert!(panics(|| {
        Rgb::from_primaries("", &zeros, WhitePoint::new(0.0, 0.0), identity, identity, 0.0, 1.0);
    }));
    assert!(panics(|| {
        Rgb::from_primaries("Test", &seven, WhitePoint::new(0.0, 0.0), identity, identity, 0.0, 1.0);
    }));
    assert!(panics(|| {
        Rgb::from_primaries("Test", &zeros, WhitePoint::new(0.0, 0.0), identity, identity, 2.0, 1.0);
    }));
    // A gamma of 0: a constant curve.
    assert!(panics(|| {
        Rgb::from_primaries_with_gamma("Test", &zeros, WhitePoint::new(0.0, 0.0), 0.0);
    }));
    assert!(panics(|| {
        TransferParameters::new(2.4, 1.0, 0.0, 1.0, 1.5, 0.0, 0.0); // d past 1
    }));
    assert!(panics(|| {
        TransferParameters::new(2.4, 1.0, 0.0, -1.0, 0.5, 0.0, 0.0); // decreasing
    }));
    assert!(panics(|| {
        TransferParameters::new(f64::NAN, 1.0, 0.0, 1.0, 0.5, 0.0, 0.0);
    }));
    // PQ: d past 1 is fine.
    assert!(!panics(|| {
        TransferParameters::new(TypePQish, -1.5, 1.8, 0.01, 18.8, -18.7, 6.2);
    }));
    // Degenerate primaries and white point are no error (the matrices are NaN).
    let test = Rgb::from_primaries("Test", &zeros, WhitePoint::new(0.0, 0.0), identity, identity, 0.0, 1.0);
    assert!(test.name() == "Test" && !test.is_srgb() && !test.is_wide_gamut());
    // The OETF and EOTF kept within the range.
    let sqrt_oetf = Rgb::from_primaries("Test", &zeros, WhitePoint::new(0.0, 0.0), |x| x.sqrt(), identity, 0.0, 1.0);
    let square_eotf = Rgb::from_primaries("Test", &zeros, WhitePoint::new(0.0, 0.0), identity, |x| x * x, 0.0, 1.0);
    assert!(near(sqrt_oetf.oetf(0.25) as f32, 0.5, 1e-5) && sqrt_oetf.oetf(4.0) == 1.0);
    assert!(near(square_eotf.eotf(0.25) as f32, 0.0625, 1e-5));
    assert!(square_eotf.eotf(3.0) == 1.0 && square_eotf.eotf(-1.0) == 0.0);
}

#[test]
fn rgb_color_spaces_turn_colors_into_xyz_and_linear_as_composes() {
    let srgb = &ColorSpaces::Srgb;
    let xyz = srgb.to_xyz(0.75, 0.5, 0.25);
    assert!(near_all(xyz, [0.3012, 0.2679, 0.0840], 1e-3));
    let mut v = [0.75, 0.5, 0.25];
    srgb.to_xyz_in_place(&mut v); // in place
    assert!(near_all(v, xyz, 1e-5));
    assert!(near_all(srgb.from_xyz(0.3012, 0.2679, 0.0840), [0.75, 0.5, 0.25], 1e-3));
    assert!(near_all(srgb.from_linear(0.0, 0.5, 1.0), [0.0, 0.7354, 1.0], 1e-3));
    assert!(near_all(srgb.to_linear(0.0, 0.5, 1.0), [0.0, 0.2140, 1.0], 1e-3));
    assert!(near_all(srgb.connect(&ColorSpaces::LinearSrgb).transform(0.5, 0.5, 0.5), [0.2140, 0.2140, 0.2140], 1e-4));
    assert!(near_all(ColorSpaces::LinearSrgb.connect(srgb).transform(0.2140, 0.5, 1.0), [0.5, 0.7354, 1.0], 1e-3));
    // CIE XYZ as it is, within -2..2.
    assert!(near_all(ColorSpaces::CieXyz.to_xyz(0.32, 0.43, 0.54), [0.32, 0.43, 0.54], 1e-7));
    assert!(near_all(ColorSpaces::CieXyz.from_xyz(3.0, -0.5, -3.0), [2.0, -0.5, -2.0], 1e-7));
    for id in 0..20 {
        let Some(rgb) = ColorSpaces::get_color_space(id).as_rgb() else { continue };
        let mut l = rgb.to_linear(0.0, 0.5, 1.0);
        rgb.from_linear_in_place(&mut l);
        assert!(near_all(l, [0.0, 0.5, 1.0], 1e-3), "{id}"); // (Compose's idempotent transfer functions)
        let mut c = [0.25, 0.5, 0.75];
        rgb.to_xyz_in_place(&mut c);
        rgb.from_xyz_in_place(&mut c);
        assert!(near_all(c, [0.25, 0.5, 0.75], 1e-4), "{id}");
    }
    assert_eq!(ColorSpaces::Srgb.transfer_parameters(), Some(srgb_parameters()));
    assert_eq!(ColorSpaces::ExtendedSrgb.transfer_parameters(), ColorSpaces::Srgb.transfer_parameters());
    // A gamma's.
    assert_eq!(
        ColorSpaces::AdobeRgb.transfer_parameters(),
        Some(TransferParameters::new(2.2, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0))
    );
    assert_eq!(ColorSpaces::Bt2020Pq.transfer_parameters().unwrap().gamma, TypePQish);
    assert_eq!(ColorSpaces::Bt2020Hlg.transfer_parameters().unwrap().gamma, TypeHLGish);
    // ExtendedSrgb keeps a negative value's sign through its transfer functions.
    assert!(near_all(ColorSpaces::ExtendedSrgb.to_linear(-0.5, 0.5, 2.0), [-0.2140, 0.2140, 4.9538], 1e-3));
}

#[test]
fn connectors_turn_colors_between_spaces_as_composes() {
    let (srgb, p3) = (&ColorSpaces::Srgb, &ColorSpaces::DciP3);
    let connector = srgb.connect(p3);
    assert!(same_space(connector.source(), srgb) && same_space(connector.destination(), p3));
    assert_eq!(connector.render_intent(), RenderIntent::Perceptual);
    let connector = srgb.connect(srgb);
    assert!(
        same_space(connector.source(), connector.destination()) && connector.render_intent() == RenderIntent::Relative
    );
    assert!(same_space(p3.connect(srgb).destination(), srgb));
    // The same white point: one matrix.
    assert!(near_all(srgb.connect(&ColorSpaces::AdobeRgb).transform(1.0, 0.5, 0.0), [0.8912, 0.4962, 0.1164], 1e-3));
    assert!(near_all(ColorSpaces::AdobeRgb.connect(srgb).transform(0.8912, 0.4962, 0.1164), [1.0, 0.5, 0.0], 1e-3));
    let mut v = [1.0, 0.5, 0.0];
    srgb.connect(&ColorSpaces::AdobeRgb).transform_in_place(&mut v);
    assert!(near_all(v, [0.8912, 0.4962, 0.1164], 1e-3));
    assert!(near_all(srgb.connect(&ColorSpaces::DisplayP3).transform(1.0, 0.0, 0.0), [0.9175, 0.2003, 0.1386], 1e-3));
    // Other white points: through D50 (Bradford), the absolute intent keeping the white point's color.
    assert!(near_all(
        srgb.connect(&ColorSpaces::ProPhotoRgb).transform(1.0, 0.0, 0.0),
        [0.70226, 0.2757, 0.1036],
        1e-4
    ));
    let relative = p3.connect_with(srgb, RenderIntent::Relative).transform(0.9, 0.9, 0.9);
    let absolute = p3.connect_with(srgb, RenderIntent::Absolute).transform(0.9, 0.9, 0.9);
    assert!(near_all(relative, [0.8862, 0.8862, 0.8862], 1e-4) && near_all(absolute, [0.8475, 0.9217, 0.8203], 1e-4));
    assert!(near_all(srgb.connect(srgb).transform(0.11112, 0.22227, 0.444448), [0.11112, 0.22227, 0.444448], 1e-5));
    assert!(near_all(p3.connect(p3).transform(1.0, 0.0, 0.0), [1.0, 0.0, 0.0], 1e-3));
    assert!(near_all(
        p3.connect(srgb).transform(0.6, 0.9, 0.7),
        p3.connect_with(srgb, RenderIntent::Perceptual).transform(0.6, 0.9, 0.7),
        1e-7
    ));
    // CIE L*a*b*, CIE XYZ and Oklab.
    let lab = ColorSpaces::CieLab.connect(srgb);
    assert!(near_all(lab.transform(100.0, 0.0, 0.0), [1.0, 1.0, 1.0], 1e-3));
    assert!(near_all(lab.transform(100.0, 0.0, 54.0), [1.0, 0.9925, 0.5762], 1e-3));
    let lab = ColorSpaces::CieLab.connect_with(srgb, RenderIntent::Absolute);
    assert!(near_all(lab.transform(100.0, 0.0, 0.0), [1.0, 0.9910, 0.8651], 1e-3));
    assert!(near_all(lab.transform(100.0, 0.0, 54.0), [1.0, 0.9853, 0.4652], 1e-3));
    assert!(near_all(srgb.connect(&ColorSpaces::CieLab).transform(1.0, 1.0, 1.0), [100.0, 0.0, 0.0], 1e-2));
    assert!(near_all(ColorSpaces::CieXyz.connect(srgb).transform(0.32, 0.43, 0.54), [0.2280, 0.7541, 0.8453], 1e-3));
    let oklab = ColorSpaces::Oklab.connect(srgb);
    assert!(near_all(oklab.transform(1.000, -0.065, 0.241), [1.0, 1.0, 0.0], 1e-3));
    assert!(near_all(oklab.transform(1.000, -0.125, -0.217), [0.0, 1.0, 1.0], 1e-3));
    assert!(near_all(oklab.transform(0.500, 0.250, 0.000), [0.768, 0.0, 0.366], 1e-3));
    assert!(near_all(oklab.transform(1.0, 0.0, 0.0), [1.0, 1.0, 1.0], 1e-3));
    assert!(near_all(oklab.transform(0.0, 0.0, 0.0), [0.0, 0.0, 0.0], 1e-3));
    assert!(near_all(srgb.connect(&ColorSpaces::Oklab).transform(1.0, 0.0, 0.0), [0.6280, 0.2249, 0.1258], 1e-3));
    let mut back = srgb.connect(&ColorSpaces::Oklab).transform(0.2, 0.4, 0.6);
    oklab.transform_in_place(&mut back);
    assert!(near_all(back, [0.2, 0.4, 0.6], 1e-4));
}

#[test]
fn connectors_of_built_in_spaces_are_kept_as_composes() {
    let (srgb, oklab) = (&ColorSpaces::Srgb, &ColorSpaces::Oklab);
    assert!(srgb.connect(srgb) == srgb.connect(srgb) && same_space(srgb.connect(srgb).source(), srgb));
    let to_oklab = srgb.connect(oklab);
    assert!(to_oklab == srgb.connect(oklab) && same_space(to_oklab.destination(), oklab));
    assert_eq!(to_oklab.render_intent(), RenderIntent::Perceptual);
    assert!(oklab.connect(srgb) == oklab.connect(srgb) && same_space(oklab.connect(srgb).destination(), srgb));
    // By the two ids and the intent.
    let p3 = ColorSpaces::DisplayP3.connect_with(srgb, RenderIntent::Absolute);
    assert!(
        p3 == ColorSpaces::DisplayP3.connect_with(srgb, RenderIntent::Absolute)
            && p3 != ColorSpaces::DisplayP3.connect(srgb)
    );
    // A user's space's: made anew each time, the same colors.
    let mine = Rgb::from_primaries_with_gamma("Mine", &SRGB_PRIMARIES_XYY, Illuminant::D50, 2.2);
    let (a, b) = (mine.connect(srgb), mine.connect(srgb));
    assert!(a != b && same_space(a.source(), &mine) && a.transform(0.3, 0.6, 0.9) == b.transform(0.3, 0.6, 0.9));
    let mut in_mine = a.transform(0.3, 0.6, 0.9);
    srgb.connect(&mine).transform_in_place(&mut in_mine);
    assert!(near_all(in_mine, [0.3, 0.6, 0.9], 1e-4));
}

#[test]
fn color_spaces_adapt_to_white_points_as_composes() {
    let srgb = &ColorSpaces::Srgb;
    let adapted = srgb.adapt(Illuminant::D50);
    let d50 = adapted.as_rgb().unwrap();
    let srgb_d50 =
        [0.43602175, 0.22247513, 0.01392813, 0.38510883, 0.71690667, 0.09710153, 0.14308129, 0.06061824, 0.71415880];
    assert!(near_all(d50.transform(), srgb_d50, 1e-7));
    assert!(near_all(
        srgb.adapt_with(Illuminant::D50, Adaptation::Bradford).as_rgb().unwrap().transform(),
        srgb_d50,
        1e-7
    ));
    assert!(d50.id() == MIN_ID && d50.name() == srgb.name() && d50.white_point() == Illuminant::D50 && !d50.is_srgb());
    assert!(d50.primaries() == srgb.primaries() && d50.transfer_parameters() == srgb.transfer_parameters());
    // Bradford's D65 to D50: the adapted matrix over sRGB's.
    let bradford =
        [1.0478112, 0.0295424, -0.0092345, 0.0228866, 0.9904844, 0.0150436, -0.0501270, -0.0170491, 0.7521316];
    let (t, inv) = (d50.transform(), srgb.inverse_transform());
    let mut m = [0.0f32; 9];
    for c in 0..3 {
        for r in 0..3 {
            m[c * 3 + r] = t[r] * inv[c * 3] + t[3 + r] * inv[c * 3 + 1] + t[6 + r] * inv[c * 3 + 2];
        }
    }
    assert!(near_all(m, bradford, 1e-4));
    // The same white point, or not RGB: the space itself.
    assert!(same_space(&*srgb.adapt(Illuminant::D65), srgb));
    assert!(same_space(&*ColorSpaces::CieLab.adapt(Illuminant::D65), &ColorSpaces::CieLab));
    // Von Kries and CIECAT02 adapt otherwise.
    let von_kries_d50 = srgb.adapt_with(Illuminant::D50, Adaptation::VonKries);
    let ciecat02_d50 = srgb.adapt_with(Illuminant::D50, Adaptation::Ciecat02);
    let (von_kries, ciecat02) = (von_kries_d50.as_rgb().unwrap(), ciecat02_d50.as_rgb().unwrap());
    assert!(!near_all(von_kries.transform(), srgb_d50, 1e-4) && !near_all(ciecat02.transform(), srgb_d50, 1e-4));
    let vk = von_kries.transform();
    assert!(near(vk[1] + vk[4] + vk[7], 1.0, 1e-5)); // white's Y still 1
}

#[test]
fn color_spaces_match_and_equal_as_composes() {
    for id in 0..20 {
        let space = ColorSpaces::get_color_space(id);
        if space.model() != ColorModel::Rgb || id == 2 || id == 3 {
            continue; // match() cannot tell the extended sRGBs
        }
        let d50 = space.adapt(Illuminant::D50);
        let rgb = d50.as_rgb().unwrap();
        assert!(
            same_space(ColorSpaces::r#match(&rgb.transform(), &rgb.transfer_parameters().unwrap()).unwrap(), space),
            "{id}"
        );
    }
    assert!(same_space(ColorSpaces::r#match(&SRGB_TO_XYZ_D50, &srgb_parameters()).unwrap(), &ColorSpaces::Srgb));
    assert!(
        ColorSpaces::r#match(&SRGB_TO_XYZ_D50, &TransferParameters::new(3.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0)).is_none()
    );
    // Equal: the same type, id, name and model, and an Rgb's white point, primaries, range and transfer parameters.
    assert!(ColorSpaces::Srgb == ColorSpaces::Srgb && ColorSpaces::Srgb != ColorSpaces::LinearSrgb);
    assert!(dyn_space(&ColorSpaces::Srgb) != dyn_space(&ColorSpaces::CieLab));
    let gamma22 = TransferParameters::new(2.2, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let a = Rgb::from_primaries_with_parameters("A", &SRGB_PRIMARIES_XYY, Illuminant::D65, gamma22);
    let b = Rgb::from_primaries_with_parameters("A", &SRGB_PRIMARIES_XYY, Illuminant::D65, gamma22);
    let c = Rgb::from_primaries_with_parameters("C", &SRGB_PRIMARIES_XYY, Illuminant::D65, gamma22);
    let d = Rgb::from_primaries_with_parameters("A", &SRGB_PRIMARIES_XYY, Illuminant::D50, gamma22);
    assert!(
        a == b
            && a != c
            && a != d
            && a == Rgb::from_primaries_with_gamma("A", &SRGB_PRIMARIES_XYY, Illuminant::D65, 2.2)
    );
    // Without transfer parameters on either side, equal whatever the functions; on one side only, by the functions
    // (Compose's).
    let f = Rgb::from_primaries("F", &SRGB_PRIMARIES_XYY, Illuminant::D65, identity, identity, 0.0, 1.0);
    let g = Rgb::from_primaries("F", &SRGB_PRIMARIES_XYY, Illuminant::D65, |x| x * x, identity, 0.0, 1.0);
    let with_parameters = Rgb::from_primaries_with_gamma("F", &SRGB_PRIMARIES_XYY, Illuminant::D65, 1.0);
    assert!(f == g && f != with_parameters && with_parameters != f);
    assert!(near(SRGB_WHITE_POINT_XYZ.x, 0.3127, 1e-4) && near(SRGB_WHITE_POINT_XYZ.y, 0.3290, 1e-4));
    assert!(
        WhitePoint::new(0.3, 0.3) == WhitePoint::new(0.3, 0.3)
            && WhitePoint::new(0.0, 0.3) != WhitePoint::new(-0.0, 0.3)
    );
}

// ---- Colors: Compose's 64 bits (ColorTest's numbers), its functions, and how it animates --------------------------

#[test]
fn colors_pack_as_composes_64_bits() {
    // sRGB: 8 bits a channel in the upper 32, a half up; its components kept within 0..1.
    assert!(
        Color::new(1.0, 0.5, 0.0, 1.0).value() == 0xFFFF8000u64 << 32
            && Color::from_argb(0xFFFF8000) == Color::new(1.0, 0.5, 0.0, 1.0)
    );
    let half_orange = kompozable_block!({ Color::hex(0xFF8000).copy(alpha = 0.5) });
    assert!(
        half_orange.value() == 0x80FF8000u64 << 32 && Color::new(0.0, 0.0, 0.0, 0.5) == Color::from_argb(0x80000000)
    );
    assert!(
        Color::new(0.0, 0.5019608, 0.0, 1.0).green() == 0.5019608 && Color::default().value() == 0xFF000000u64 << 32
    );
    assert!(Color::new(2.0, 0.0, 0.0, 1.0).red() == 1.0 && Color::new(-1.0, 0.0, 0.0, 1.0).red() == 0.0);
    assert!(Color::new(0.0, 2.0, 0.0, 1.0).green() == 1.0 && Color::new(0.0, -1.0, 0.0, 1.0).green() == 0.0);
    assert!(Color::new(0.0, 0.0, 2.0, 1.0).blue() == 1.0 && Color::new(0.0, 0.0, -1.0, 1.0).blue() == 0.0);
    assert!(Color::new(0.0, 0.0, 0.0, 2.0).alpha() == 1.0 && Color::new(0.0, 0.0, 0.0, -1.0).alpha() == 0.0);
    let red = Color::new(1.0, 0.0, 0.0, 1.0);
    assert!(
        Color::new(f32::NAN, 0.0, 0.0, 1.0).red() == 0.0
            && red.is_srgb()
            && same_space(red.color_space(), &ColorSpaces::Srgb)
    );
    // Another space: half floats at 48, 32 and 16, a 10-bit alpha at 6, the space's id.
    let p3 = Color::new_in(0.15, 0.2, 0.3, 0.5, &ColorSpaces::DisplayP3);
    let half = |v: f32| float_to_half(v) as u64;
    assert_eq!(p3.value(), half(0.15) << 48 | half(0.2) << 32 | half(0.3) << 16 | 512 << 6 | 7);
    assert!(near(p3.red(), 0.15, 0.0005) && near(p3.green(), 0.2, 0.0005) && near(p3.blue(), 0.3, 0.0005));
    assert_eq!(p3.alpha(), 512.0 / 1023.0);
    assert!(
        !p3.is_srgb()
            && p3.color_space().equals(&ColorSpaces::DisplayP3)
            && same_space(p3.color_space(), &ColorSpaces::DisplayP3)
    );
    let aces = Color::new_in(0.2, 0.3, 0.4, 0.6, &ColorSpaces::Aces);
    assert!(near(aces.red(), 0.2, 0.0005) && near(aces.green(), 0.3, 0.0005) && near(aces.blue(), 0.4, 0.0005));
    // Kept within the space's range: Display P3's 0..1, CIE Lab's L 0..100 and a, b -128..128.
    let out = Color::new_in(2.0, -1.0, 0.5, 3.0, &ColorSpaces::DisplayP3);
    let lab = Color::new_in(150.0, -200.0, 64.0, 1.0, &ColorSpaces::CieLab);
    assert!(out.red() == 1.0 && out.green() == 0.0 && out.blue() == 0.5 && out.alpha() == 1.0);
    assert!(
        lab.red() == 100.0
            && lab.green() == -128.0
            && lab.blue() == 64.0
            && lab.color_space().equals(&ColorSpaces::CieLab)
    );
    // A space of a built-in's numbers is sRGB if it is sRGB; another needs an id.
    let mine = Rgb::from_primaries_with_parameters("Mine", &SRGB_PRIMARIES_XYY, Illuminant::D65, srgb_parameters());
    assert_eq!(Color::new_in(1.0, 0.5, 0.0, 1.0, &mine), Color::new(1.0, 0.5, 0.0, 1.0));
    let unknown =
        Rgb::from_primaries_with_gamma("Unknown", &[0.680, 0.320, 0.265, 0.690, 0.150, 0.060], Illuminant::D65, 2.2);
    assert!(panics(|| {
        Color::new_in(1.0, 0.0, 0.0, 1.0, &unknown);
    }));
    // copy: another alpha, the components as they are; or other components, in its space.
    let opaque = kompozable_block!({ p3.copy(alpha = 1.0) });
    assert!(opaque.red() == p3.red() && opaque.alpha() == 1.0 && opaque.color_space().equals(&ColorSpaces::DisplayP3));
    let orange = Color::new(1.0, 0.5, 0.0, 1.0);
    assert_eq!(kompozable_block!({ orange.copy(alpha = 0.5) }), Color::from_argb(0x80FF8000));
    assert_eq!(kompozable_block!({ orange.copy(alpha = f32::NAN) }).alpha(), 0.0);
    assert_eq!(orange.copy(), orange); // (nothing given: the color itself)
    let other = kompozable_block!({ p3.copy(alpha = 0.25, red = 0.5, green = 0.75, blue = 1.0) });
    assert!(
        other.color_space().equals(&ColorSpaces::DisplayP3)
            && other.red() == 0.5
            && other.green() == 0.75
            && other.blue() == 1.0
    );
    assert_eq!(other.alpha(), 256.0 / 1023.0);
    // Unspecified: 0x10, no color in ColorSpaces::Unspecified, none of Transparent's.
    let unspecified = Color::Unspecified;
    assert!(unspecified.value() == 0x10 && unspecified.is_unspecified() && !unspecified.is_specified());
    assert!(Color::Red.is_specified() && !Color::Red.is_unspecified() && unspecified != Color::Transparent);
    assert_eq!(Color::new_in(0.0, 0.0, 0.0, 0.0, &ColorSpaces::Unspecified), unspecified);
    assert_eq!(Color::Transparent.to_argb(), unspecified.to_argb());
    assert_eq!(unspecified.take_or_else(|| Color::Red), Color::Red);
    assert_eq!(format!("{:?}", Color::Red), "Color(1.0, 0.0, 0.0, 1.0, sRGB IEC61966-2.1)");
}

#[test]
fn colors_of_other_spaces_are_taken_in_srgb() {
    // What takes a color as sRGB takes another space's in sRGB (HCT); blending, color filters and bitmaps are
    // the brush's and the image's.
    let p3 = Color::new_in(0.5, 0.2, 0.8, 1.0, &ColorSpaces::DisplayP3);
    let srgb = p3.convert(&ColorSpaces::Srgb);
    let raw = Color::new(p3.red(), p3.green(), p3.blue(), 1.0); // (its numbers taken as sRGB's: another color)
    assert_ne!(srgb, raw);
    assert!(Hct::from_color(p3) == Hct::from_color(srgb) && Hct::from_color(p3) != Hct::from_color(raw));
    assert_eq!(DynamicTonalPalette::from_seed(p3), DynamicTonalPalette::from_seed(srgb));
    assert!(p3.in_srgb() == srgb && srgb.in_srgb() == srgb);
    let c = p3.srgb_components();
    assert!(near(c[0], srgb.red(), 1.0 / 255.0) && near(c[1], srgb.green(), 1.0 / 255.0) && c[3] == 1.0);
    // Past sRGB, in extended sRGB: a P3 red's red over 1, its green under 0.
    let p3_red = Color::new_in(1.0, 0.0, 0.0, 1.0, &ColorSpaces::DisplayP3).srgb_components();
    assert!(p3_red[0] > 1.0 && p3_red[1] < 0.0);
}

#[test]
fn colors_keep_their_components_as_composes_half_floats() {
    assert!(float_to_half(1.0) == 0x3c00 && float_to_half(0.5) == 0x3800 && float_to_half(-2.0) == 0xc000);
    assert!(float_to_half(0.1) == 0x2e66 && float_to_half(1.0 / 3.0) == 0x3555 && float_to_half(65504.0) == 0x7bff);
    assert!(float_to_half(0.0) == 0 && float_to_half(-0.0) == 0x8000 && float_to_half(f32::INFINITY) == 0x7c00);
    assert_eq!(float_to_half(f32::NAN), 0x7e00);
    assert!(float_to_half(1e-7) == 0x0002 && float_to_half(1e-9) == 0); // subnormal; under the smallest
    assert_eq!(float_to_half(1e6), 0xc400); // (Compose's overflow, bit for bit: an exponent of 0x31)
    assert!(half_to_float(0x3c00) == 1.0 && half_to_float(0x3555) == 0.333251953125 && half_to_float(0xc000) == -2.0);
    assert!(
        half_to_float(0x0001) == 5.9604645e-8
            && half_to_float(0x8001) == -5.9604645e-8
            && half_to_float(0x7bff) == 65504.0
    );
    assert!(
        half_to_float(0x7c00) == f32::INFINITY
            && half_to_float(0x7e00).is_nan()
            && half_to_float(0x8000).is_sign_negative()
    );
    // Every finite half back to itself.
    for h in 0..=0xffffu16 {
        if h & 0x7c00 != 0x7c00 {
            assert_eq!(float_to_half(half_to_float(h)), h);
        }
    }
}

#[test]
fn colors_convert_composite_and_measure_as_composes() {
    let srgb = Color::from_argb(0xFFFF8000);
    let adobe = Color::new_in(0.8916, 0.4980, 0.1168, 1.0, &ColorSpaces::AdobeRgb);
    let converted = srgb.convert(&ColorSpaces::AdobeRgb);
    assert!(converted.color_space().equals(&ColorSpaces::AdobeRgb) && near(converted.red(), adobe.red(), 0.0005));
    assert!(
        near(converted.green(), adobe.green(), 0.0005)
            && near(converted.blue(), adobe.blue(), 0.0005)
            && converted.alpha() == 1.0
    );
    assert!(srgb.to_argb() == 0xFFFF8000 && adobe.to_argb() == 0xFFFF8000 && srgb.convert(&ColorSpaces::Srgb) == srgb);
    let green = Color::from_argb(0xFF64B432);
    let back = green.convert(&ColorSpaces::CieLab).convert(&ColorSpaces::Srgb);
    assert!(
        near(back.red(), 100.0 / 255.0, 0.01)
            && near(back.green(), 180.0 / 255.0, 0.01)
            && near(back.blue(), 50.0 / 255.0, 0.01)
    );
    assert_eq!(
        Color::from_argb(0x1F2B3D4E).convert(&ColorSpaces::Oklab).convert(&ColorSpaces::Srgb).to_argb(),
        0x1F2B3D4E
    );
    // composite_over: in the background's space.
    let same = Color::from_argb(0x7f007f00).composite_over(Color::from_argb(0x7f7f0000));
    assert!(near(same.red(), 0.16, 0.01) && near(same.green(), 0.33, 0.01) && near(same.blue(), 0.0, 0.01));
    assert!(near(same.alpha(), 0.75, 0.01));
    let p3 = Color::from_argb(0x7f007f00).composite_over(Color::new_in(0.5, 0.0, 0.0, 0.5, &ColorSpaces::DisplayP3));
    assert!(
        p3.color_space().equals(&ColorSpaces::DisplayP3) && near(p3.red(), 0.31, 0.01) && near(p3.green(), 0.33, 0.01)
    );
    assert!(near(p3.blue(), 0.09, 0.01) && near(p3.alpha(), 0.75, 0.01));
    let low = Color::new(0.0, 1.0, 0.0, 0.01).composite_over(Color::new(1.0, 0.0, 0.0, 0.01));
    assert!(
        near(low.red(), 0.5, 0.01)
            && near(low.green(), 0.5, 0.01)
            && near(low.blue(), 0.0, 0.01)
            && near(low.alpha(), 0.01, 0.02)
    );
    assert_eq!(
        Color::from_argb(0x00007f00).composite_over(Color::from_argb(0x007f0000)),
        Color::new(0.0, 0.0, 0.0, 0.0)
    );
    // Of other alphas, each a half up in 8 bits: a quarter green over red; a half over a quarter.
    assert_eq!(Color::new(0.0, 1.0, 0.0, 0.25).composite_over(Color::Red), Color::from_argb(0xFFBF4000));
    assert_eq!(
        Color::new(0.0, 1.0, 0.0, 0.5).composite_over(Color::new(1.0, 0.0, 0.0, 0.25)),
        Color::from_argb(0xA033CC00)
    );
    // luminance: an RGB space's, its EOTF's.
    assert!(
        Color::Black.luminance() == 0.0
            && near(Color::Blue.luminance(), 0.0722, 0.0005)
            && near(Color::Red.luminance(), 0.2126, 0.0005)
    );
    assert!(near(Color::Green.luminance(), 0.7152, 0.0005) && Color::White.luminance() == 1.0);
    assert!(near(Color::from_argb(0xFF808080).luminance(), 0.2159, 0.0005)); // (0x80 is a fifth of the light)
    assert!(near(Color::new_in(1.0, 1.0, 1.0, 1.0, &ColorSpaces::LinearSrgb).luminance(), 1.0, 1e-3));
    assert!(near(Color::new_in(0.5, 0.5, 0.5, 1.0, &ColorSpaces::LinearSrgb).luminance(), 0.5, 1e-3));
    assert!(panics(|| {
        Color::new_in(50.0, 0.0, 0.0, 1.0, &ColorSpaces::CieLab).luminance();
    }));
}

#[test]
fn colors_from_hsv_and_hsl_as_composes() {
    let gray = Color::from_argb(0xFF808080);
    assert!(Color::hsv(0.0, 0.0, 0.0, 0.0) == Color::Transparent && Color::hsv(0.0, 0.0, 0.0, 1.0) == Color::Black);
    assert_eq!(Color::hsv(120.0, 1.0, 0.0, 1.0), Color::Black);
    assert!(Color::hsv(0.0, 0.0, 1.0, 1.0) == Color::White && Color::hsv(240.0, 0.0, 1.0, 1.0) == Color::White);
    assert_eq!(Color::hsv(120.0, 0.0, 0.5, 1.0), gray);
    assert!(Color::hsv(0.0, 1.0, 1.0, 1.0) == Color::Red && Color::hsv(60.0, 1.0, 1.0, 1.0) == Color::Yellow);
    assert!(Color::hsv(120.0, 1.0, 1.0, 1.0) == Color::Green && Color::hsv(180.0, 1.0, 1.0, 1.0) == Color::Cyan);
    assert!(Color::hsv(240.0, 1.0, 1.0, 1.0) == Color::Blue && Color::hsv(300.0, 1.0, 1.0, 1.0) == Color::Magenta);
    assert_eq!(Color::hsv(360.0, 1.0, 1.0, 1.0), Color::Red);
    assert!(Color::hsl(0.0, 0.0, 0.0, 0.0) == Color::Transparent && Color::hsl(120.0, 1.0, 0.0, 1.0) == Color::Black);
    assert!(Color::hsl(120.0, 1.0, 1.0, 1.0) == Color::White && Color::hsl(240.0, 0.5, 1.0, 1.0) == Color::White);
    assert_eq!(Color::hsl(240.0, 0.0, 0.5, 1.0), gray);
    assert!(Color::hsl(0.0, 1.0, 0.5, 1.0) == Color::Red && Color::hsl(60.0, 1.0, 0.5, 1.0) == Color::Yellow);
    assert!(Color::hsl(120.0, 1.0, 0.5, 1.0) == Color::Green && Color::hsl(180.0, 1.0, 0.5, 1.0) == Color::Cyan);
    assert!(Color::hsl(240.0, 1.0, 0.5, 1.0) == Color::Blue && Color::hsl(300.0, 1.0, 0.5, 1.0) == Color::Magenta);
    assert_eq!(Color::hsl(360.0, 1.0, 0.5, 1.0), Color::Red);
    let linear = &ColorSpaces::LinearSrgb;
    assert_eq!(Color::hsv_in(120.0, 0.0, 1.0, 1.0, linear).convert(&ColorSpaces::Srgb), Color::White);
    assert_eq!(Color::hsv_in(240.0, 0.0, 0.5, 1.0, linear), Color::new_in(0.5, 0.5, 0.5, 1.0, linear));
    assert_eq!(Color::hsv_in(60.0, 1.0, 1.0, 1.0, linear), Color::new_in(1.0, 1.0, 0.0, 1.0, linear));
    assert_eq!(Color::hsv_in(300.0, 1.0, 1.0, 1.0, linear), Color::new_in(1.0, 0.0, 1.0, 1.0, linear));
    assert_eq!(Color::hsl_in(180.0, 1.0, 0.5, 1.0, linear), Color::new_in(0.0, 1.0, 1.0, 1.0, linear));
    assert_eq!(Color::hsl_in(0.0, 0.0, 0.5, 1.0, linear), Color::new_in(0.5, 0.5, 0.5, 1.0, linear));
    let threw = [
        panics(|| {
            Color::hsv(361.0, 0.0, 0.0, 1.0);
        }),
        panics(|| {
            Color::hsv(0.0, 1.5, 0.0, 1.0);
        }),
        panics(|| {
            Color::hsv(0.0, 0.0, -1.0, 1.0);
        }),
        panics(|| {
            Color::hsl(-1.0, 0.0, 0.0, 1.0);
        }),
        panics(|| {
            Color::hsl(0.0, 0.0, 2.0, 1.0);
        }),
        panics(|| {
            Color::hsv(f32::NAN, 0.0, 0.0, 1.0);
        }),
    ];
    assert!(threw.iter().all(|&t| t));
}

/// ColorTest's: red to blue at a thousandth at a time, as Compose makes them.
#[rustfmt::skip]
const RED_TO_BLUE: [u32; 1001] = [
    0xff0000, 0xff0102, 0xfe0203, 0xfe0305, 0xfe0507, 0xfe0609, 0xfe070a, 0xfd080c, 0xfd090d, 0xfd0b0f,
    0xfd0b10, 0xfc0d11, 0xfc0e13, 0xfc0e13, 0xfc1015, 0xfc1116, 0xfb1117, 0xfb1218, 0xfb1319, 0xfb1319,
    0xfa141a, 0xfa151b, 0xfa151c, 0xfa161d, 0xf9171d, 0xf9181e, 0xf9181f, 0xf91920, 0xf91920, 0xf81a21,
    0xf81a22, 0xf81b22, 0xf81b23, 0xf71c24, 0xf71d24, 0xf71d25, 0xf71d26, 0xf71e26, 0xf61e27, 0xf61f27,
    0xf61f28, 0xf62028, 0xf52029, 0xf5212a, 0xf5212a, 0xf5222b, 0xf4222b, 0xf4222c, 0xf4232c, 0xf4232d,
    0xf4232d, 0xf3242e, 0xf3242e, 0xf3252f, 0xf3252f, 0xf22530, 0xf22630, 0xf22631, 0xf22631, 0xf22732,
    0xf12732, 0xf12833, 0xf12833, 0xf12833, 0xf02934, 0xf02935, 0xf02935, 0xf02935, 0xef2a36, 0xef2a36,
    0xef2b37, 0xef2b37, 0xef2b37, 0xee2b38, 0xee2c38, 0xee2c39, 0xee2c39, 0xed2c3a, 0xed2d3a, 0xed2d3a,
    0xed2d3b, 0xed2e3b, 0xec2e3b, 0xec2e3c, 0xec2f3c, 0xec2f3d, 0xeb2f3d, 0xeb2f3e, 0xeb2f3e, 0xeb303e,
    0xea303f, 0xea303f, 0xea313f, 0xea3140, 0xea3140, 0xe93141, 0xe93141, 0xe93241, 0xe93242, 0xe83242,
    0xe83342, 0xe83343, 0xe83343, 0xe73343, 0xe73344, 0xe73444, 0xe73444, 0xe63445, 0xe63445, 0xe63546,
    0xe63546, 0xe63546, 0xe53547, 0xe53547, 0xe53647, 0xe53648, 0xe43648, 0xe43648, 0xe43648, 0xe43749,
    0xe43749, 0xe33749, 0xe3374a, 0xe3384a, 0xe3384a, 0xe2384b, 0xe2384b, 0xe2384b, 0xe2384c, 0xe1394c,
    0xe1394c, 0xe1394d, 0xe1394d, 0xe1394d, 0xe03a4e, 0xe03a4e, 0xe03a4e, 0xe03a4f, 0xdf3a4f, 0xdf3b4f,
    0xdf3b50, 0xdf3b50, 0xdf3b50, 0xde3b50, 0xde3b51, 0xde3c51, 0xde3c51, 0xdd3c52, 0xdd3c52, 0xdd3c52,
    0xdd3c53, 0xdd3d53, 0xdc3d53, 0xdc3d53, 0xdc3d54, 0xdc3d54, 0xdb3d54, 0xdb3d55, 0xdb3e55, 0xdb3e55,
    0xda3e55, 0xda3e56, 0xda3e56, 0xda3e56, 0xda3f57, 0xd93f57, 0xd93f57, 0xd93f57, 0xd93f58, 0xd83f58,
    0xd84058, 0xd84059, 0xd84059, 0xd84059, 0xd74059, 0xd7405a, 0xd7405a, 0xd7415a, 0xd6415b, 0xd6415b,
    0xd6415b, 0xd6415b, 0xd5415c, 0xd5415c, 0xd5425c, 0xd5425c, 0xd5425d, 0xd4425d, 0xd4425d, 0xd4425e,
    0xd4425e, 0xd3425e, 0xd3435e, 0xd3435f, 0xd3435f, 0xd2435f, 0xd2435f, 0xd24360, 0xd24360, 0xd24360,
    0xd14460, 0xd14461, 0xd14461, 0xd14461, 0xd04461, 0xd04462, 0xd04462, 0xd04462, 0xcf4562, 0xcf4563,
    0xcf4563, 0xcf4563, 0xcf4564, 0xce4564, 0xce4564, 0xce4564, 0xce4565, 0xcd4665, 0xcd4665, 0xcd4665,
    0xcd4666, 0xcc4666, 0xcc4666, 0xcc4666, 0xcc4667, 0xcc4767, 0xcb4767, 0xcb4767, 0xcb4768, 0xcb4768,
    0xca4768, 0xca4768, 0xca4769, 0xca4769, 0xca4769, 0xc94769, 0xc9486a, 0xc9486a, 0xc9486a, 0xc8486a,
    0xc8486a, 0xc8486b, 0xc8486b, 0xc7486b, 0xc7486c, 0xc7496c, 0xc7496c, 0xc7496c, 0xc6496d, 0xc6496d,
    0xc6496d, 0xc6496d, 0xc5496d, 0xc5496e, 0xc5496e, 0xc5496e, 0xc54a6e, 0xc44a6f, 0xc44a6f, 0xc44a6f,
    0xc44a6f, 0xc34a70, 0xc34a70, 0xc34a70, 0xc34a70, 0xc24a70, 0xc24a71, 0xc24b71, 0xc24b71, 0xc24b71,
    0xc14b72, 0xc14b72, 0xc14b72, 0xc14b72, 0xc04b73, 0xc04b73, 0xc04b73, 0xc04b73, 0xc04b74, 0xbf4b74,
    0xbf4c74, 0xbf4c74, 0xbf4c74, 0xbe4c75, 0xbe4c75, 0xbe4c75, 0xbe4c75, 0xbd4c76, 0xbd4c76, 0xbd4c76,
    0xbd4c76, 0xbc4c77, 0xbc4c77, 0xbc4c77, 0xbc4d77, 0xbc4d78, 0xbb4d78, 0xbb4d78, 0xbb4d78, 0xbb4d78,
    0xba4d79, 0xba4d79, 0xba4d79, 0xba4d79, 0xb94d79, 0xb94d7a, 0xb94d7a, 0xb94d7a, 0xb94e7a, 0xb84e7b,
    0xb84e7b, 0xb84e7b, 0xb84e7b, 0xb74e7b, 0xb74e7c, 0xb74e7c, 0xb74e7c, 0xb74e7c, 0xb64e7d, 0xb64e7d,
    0xb64e7d, 0xb64e7d, 0xb54e7e, 0xb54f7e, 0xb54e7e, 0xb54f7e, 0xb44f7e, 0xb44f7f, 0xb44f7f, 0xb44f7f,
    0xb44f7f, 0xb34f80, 0xb34f80, 0xb34f80, 0xb34f80, 0xb24f80, 0xb24f81, 0xb24f81, 0xb24f81, 0xb24f81,
    0xb14f81, 0xb14f82, 0xb15082, 0xb15082, 0xb05082, 0xb05083, 0xb05083, 0xb05083, 0xb05083, 0xaf5083,
    0xaf5084, 0xaf5084, 0xae5084, 0xae5084, 0xae5084, 0xae5085, 0xae5085, 0xad5085, 0xad5085, 0xad5086,
    0xad5086, 0xac5086, 0xac5086, 0xac5086, 0xac5187, 0xab5087, 0xab5187, 0xab5187, 0xab5187, 0xab5188,
    0xaa5188, 0xaa5188, 0xaa5188, 0xaa5189, 0xa95189, 0xa95189, 0xa95189, 0xa95189, 0xa9518a, 0xa8518a,
    0xa8518a, 0xa8518a, 0xa8518a, 0xa7518b, 0xa7518b, 0xa7518b, 0xa7518b, 0xa6518b, 0xa6518c, 0xa6518c,
    0xa6518c, 0xa6528c, 0xa5528d, 0xa5528d, 0xa5528d, 0xa5528d, 0xa4528d, 0xa4528e, 0xa4528e, 0xa4528e,
    0xa4528e, 0xa3528e, 0xa3528f, 0xa3528f, 0xa2528f, 0xa2528f, 0xa2528f, 0xa25290, 0xa25290, 0xa15290,
    0xa15290, 0xa15290, 0xa15291, 0xa05291, 0xa05291, 0xa05291, 0xa05291, 0xa05292, 0x9f5292, 0x9f5292,
    0x9f5292, 0x9f5292, 0x9e5293, 0x9e5293, 0x9e5293, 0x9e5293, 0x9d5293, 0x9d5294, 0x9d5394, 0x9d5294,
    0x9d5394, 0x9c5395, 0x9c5395, 0x9c5395, 0x9c5395, 0x9b5395, 0x9b5396, 0x9b5396, 0x9b5396, 0x9b5396,
    0x9a5396, 0x9a5397, 0x9a5397, 0x9a5397, 0x995397, 0x995397, 0x995398, 0x995398, 0x985398, 0x985398,
    0x985398, 0x985398, 0x975399, 0x975399, 0x975399, 0x975399, 0x97539a, 0x96539a, 0x96539a, 0x96539a,
    0x96539a, 0x95539b, 0x95539b, 0x95539b, 0x95539b, 0x94539b, 0x94539c, 0x94539c, 0x94539c, 0x94539c,
    0x93539c, 0x93539d, 0x93539d, 0x93539d, 0x92539d, 0x92539d, 0x92539d, 0x92539e, 0x92539e, 0x91539e,
    0x91539e, 0x91539f, 0x90539f, 0x90539f, 0x90539f, 0x90539f, 0x9053a0, 0x8f53a0, 0x8f53a0, 0x8f53a0,
    0x8f53a0, 0x8e53a1, 0x8e53a1, 0x8e53a1, 0x8e53a1, 0x8e53a1, 0x8d53a1, 0x8d53a2, 0x8d53a2, 0x8d53a2,
    0x8c53a2, 0x8c53a2, 0x8c53a3, 0x8c53a3, 0x8b53a3, 0x8b53a3, 0x8b53a4, 0x8b53a4, 0x8b53a4, 0x8a53a4,
    0x8a53a4, 0x8a53a4, 0x8a53a5, 0x8953a5, 0x8953a5, 0x8953a5, 0x8953a5, 0x8853a6, 0x8853a6, 0x8853a6,
    0x8853a6, 0x8753a6, 0x8753a7, 0x8753a7, 0x8753a7, 0x8753a7, 0x8653a7, 0x8653a8, 0x8653a8, 0x8653a8,
    0x8553a8, 0x8553a8, 0x8553a8, 0x8553a9, 0x8453a9, 0x8453a9, 0x8453a9, 0x8453a9, 0x8353aa, 0x8353aa,
    0x8353aa, 0x8353aa, 0x8353ab, 0x8252ab, 0x8253ab, 0x8253ab, 0x8252ab, 0x8152ab, 0x8152ac, 0x8152ac,
    0x8152ac, 0x8052ac, 0x8052ac, 0x8052ad, 0x8052ad, 0x8052ad, 0x7f52ad, 0x7f52ad, 0x7f52ae, 0x7f52ae,
    0x7e52ae, 0x7e52ae, 0x7e52ae, 0x7e52ae, 0x7d52af, 0x7d52af, 0x7d52af, 0x7d52af, 0x7c52af, 0x7c52b0,
    0x7c52b0, 0x7c52b0, 0x7c52b0, 0x7b52b0, 0x7b52b1, 0x7b52b1, 0x7b52b1, 0x7a52b1, 0x7a52b1, 0x7a52b2,
    0x7a52b2, 0x7952b2, 0x7952b2, 0x7952b2, 0x7951b2, 0x7851b3, 0x7851b3, 0x7851b3, 0x7851b3, 0x7851b4,
    0x7751b4, 0x7751b4, 0x7751b4, 0x7751b4, 0x7651b4, 0x7651b5, 0x7651b5, 0x7651b5, 0x7551b5, 0x7551b5,
    0x7551b6, 0x7551b6, 0x7451b6, 0x7451b6, 0x7451b6, 0x7451b7, 0x7451b7, 0x7351b7, 0x7351b7, 0x7351b7,
    0x7351b7, 0x7251b8, 0x7250b8, 0x7250b8, 0x7250b8, 0x7150b8, 0x7150b9, 0x7150b9, 0x7150b9, 0x7050b9,
    0x7050b9, 0x7050b9, 0x7050ba, 0x6f50ba, 0x6f50ba, 0x6f50ba, 0x6f50ba, 0x6f50bb, 0x6e50bb, 0x6e50bb,
    0x6e50bb, 0x6e50bb, 0x6d50bc, 0x6d50bc, 0x6d4fbc, 0x6d4fbc, 0x6c4fbc, 0x6c4fbc, 0x6c4fbd, 0x6c4fbd,
    0x6b4fbd, 0x6b4fbd, 0x6b4fbe, 0x6b4fbe, 0x6a4fbe, 0x6a4fbe, 0x6a4fbe, 0x6a4fbe, 0x694fbf, 0x694fbf,
    0x694fbf, 0x694fbf, 0x694fbf, 0x684fc0, 0x684ec0, 0x684ec0, 0x684ec0, 0x674ec0, 0x674ec1, 0x674ec1,
    0x674ec1, 0x664ec1, 0x664ec1, 0x664ec1, 0x664ec2, 0x654ec2, 0x654ec2, 0x654ec2, 0x654ec2, 0x654ec3,
    0x644dc3, 0x644dc3, 0x644dc3, 0x634dc3, 0x634dc3, 0x634dc4, 0x634dc4, 0x634dc4, 0x624dc4, 0x624dc4,
    0x624dc5, 0x624dc5, 0x614dc5, 0x614dc5, 0x614cc5, 0x614cc6, 0x604cc6, 0x604cc6, 0x604cc6, 0x604cc6,
    0x5f4cc6, 0x5f4cc7, 0x5f4cc7, 0x5f4cc7, 0x5e4cc7, 0x5e4cc7, 0x5e4cc8, 0x5e4cc8, 0x5d4cc8, 0x5d4bc8,
    0x5d4bc8, 0x5d4bc8, 0x5c4bc9, 0x5c4bc9, 0x5c4bc9, 0x5c4bc9, 0x5b4bc9, 0x5b4bca, 0x5b4bca, 0x5b4bca,
    0x5b4bca, 0x5a4bca, 0x5a4aca, 0x5a4acb, 0x5a4acb, 0x594acb, 0x594acb, 0x594acb, 0x594acc, 0x584acc,
    0x584acc, 0x584acc, 0x584acc, 0x574acc, 0x5749cd, 0x5749cd, 0x5749cd, 0x5649cd, 0x5649cd, 0x5649ce,
    0x5649ce, 0x5549ce, 0x5549ce, 0x5549ce, 0x5549cf, 0x5449cf, 0x5448cf, 0x5448cf, 0x5448cf, 0x5348cf,
    0x5348d0, 0x5348d0, 0x5348d0, 0x5248d0, 0x5248d0, 0x5248d1, 0x5247d1, 0x5147d1, 0x5147d1, 0x5147d1,
    0x5147d1, 0x5047d2, 0x5047d2, 0x5047d2, 0x5047d2, 0x4f47d2, 0x4f46d3, 0x4f46d3, 0x4f46d3, 0x4e46d3,
    0x4e46d3, 0x4e46d4, 0x4e46d4, 0x4d46d4, 0x4d46d4, 0x4d46d4, 0x4d45d4, 0x4c45d5, 0x4c45d5, 0x4c45d5,
    0x4c45d5, 0x4b45d5, 0x4b45d5, 0x4b45d6, 0x4b45d6, 0x4a44d6, 0x4a44d6, 0x4a44d6, 0x4a44d7, 0x4944d7,
    0x4944d7, 0x4944d7, 0x4944d7, 0x4843d8, 0x4843d8, 0x4843d8, 0x4843d8, 0x4743d8, 0x4743d9, 0x4743d9,
    0x4743d9, 0x4643d9, 0x4642d9, 0x4642d9, 0x4642da, 0x4542da, 0x4542da, 0x4542da, 0x4542da, 0x4442da,
    0x4441db, 0x4441db, 0x4341db, 0x4341db, 0x4341db, 0x4341dc, 0x4241dc, 0x4241dc, 0x4240dc, 0x4240dc,
    0x4140dc, 0x4140dd, 0x4140dd, 0x4140dd, 0x4040dd, 0x403fdd, 0x403fde, 0x403fde, 0x3f3fde, 0x3f3fde,
    0x3f3fde, 0x3f3fdf, 0x3e3edf, 0x3e3edf, 0x3e3edf, 0x3d3edf, 0x3d3edf, 0x3d3ee0, 0x3d3ee0, 0x3c3de0,
    0x3c3de0, 0x3c3de0, 0x3c3de0, 0x3b3de1, 0x3b3de1, 0x3b3de1, 0x3b3ce1, 0x3a3ce1, 0x3a3ce2, 0x3a3ce2,
    0x393ce2, 0x393ce2, 0x393be2, 0x393be3, 0x383be3, 0x383be3, 0x383be3, 0x383be3, 0x373ae3, 0x373ae4,
    0x373ae4, 0x363ae4, 0x363ae4, 0x363ae4, 0x3639e4, 0x3539e5, 0x3539e5, 0x3539e5, 0x3539e5, 0x3439e5,
    0x3438e6, 0x3438e6, 0x3338e6, 0x3338e6, 0x3338e6, 0x3337e6, 0x3237e7, 0x3237e7, 0x3237e7, 0x3137e7,
    0x3137e7, 0x3136e8, 0x3136e8, 0x3036e8, 0x3036e8, 0x3036e8, 0x3035e8, 0x2f35e9, 0x2f35e9, 0x2f35e9,
    0x2e35e9, 0x2e34e9, 0x2e34ea, 0x2e34ea, 0x2d34ea, 0x2d34ea, 0x2d33ea, 0x2c33eb, 0x2c33eb, 0x2c33eb,
    0x2b32eb, 0x2b32eb, 0x2b32eb, 0x2b32ec, 0x2a32ec, 0x2a31ec, 0x2a31ec, 0x2931ec, 0x2931ec, 0x2931ed,
    0x2930ed, 0x2830ed, 0x2830ed, 0x2830ed, 0x272fee, 0x272fee, 0x272fee, 0x262fee, 0x262eee, 0x262eef,
    0x252eef, 0x252def, 0x252def, 0x252def, 0x242def, 0x242cf0, 0x242cf0, 0x232cf0, 0x232cf0, 0x232bf0,
    0x222bf1, 0x222bf1, 0x222bf1, 0x212af1, 0x212af1, 0x212af1, 0x202af1, 0x2029f2, 0x2029f2, 0x1f29f2,
    0x1f28f2, 0x1f28f2, 0x1e28f3, 0x1e27f3, 0x1e27f3, 0x1d27f3, 0x1d27f3, 0x1d26f3, 0x1c26f4, 0x1c25f4,
    0x1c25f4, 0x1b25f4, 0x1b25f4, 0x1b24f5, 0x1a24f5, 0x1a24f5, 0x1923f5, 0x1923f5, 0x1922f6, 0x1822f6,
    0x1822f6, 0x1821f6, 0x1721f6, 0x1720f6, 0x1620f7, 0x1620f7, 0x161ff7, 0x151ff7, 0x151ef7, 0x141ef8,
    0x141ef8, 0x141df8, 0x131df8, 0x131cf8, 0x121cf8, 0x121bf9, 0x111bf9, 0x111af9, 0x111af9, 0x1019f9,
    0x1019fa, 0x0f18fa, 0x0f18fa, 0x0e17fa, 0x0e17fa, 0x0d16fa, 0x0d15fb, 0x0c15fb, 0x0c14fb, 0x0b14fb,
    0x0b13fb, 0x0a12fc, 0x0a11fc, 0x0911fc, 0x0810fc, 0x080ffc, 0x070efc, 0x070dfd, 0x060dfd, 0x060cfd,
    0x050bfd, 0x0509fd, 0x0409fe, 0x0308fe, 0x0307fe, 0x0206fe, 0x0204fe, 0x0103fe, 0x0102fe, 0x0001ff,
    0x0000ff,
];

#[test]
fn colors_lerp_in_oklab_as_composes() {
    let (mut off, mut worst) = (0, 0);
    for (i, &expected) in RED_TO_BLUE.iter().enumerate() {
        let got = lerp_color(Color::Red, Color::Blue, i as f32 / 1000.0).to_argb();
        let d =
            [0, 8, 16].iter().map(|&s| ((got >> s & 0xff) as i32 - (expected >> s & 0xff) as i32).abs()).max().unwrap();
        off += (d != 0) as i32;
        worst = worst.max(d);
        assert_eq!(got >> 24, 0xff);
    }
    assert!(worst < 2 && off <= 1, "worst {worst}, off {off}"); // (as ColorTest allows, within a unit)
    // As ColorTest works it out: each Oklab component along, back in sRGB; alpha along too.
    let red_oklab = Color::Red.convert(&ColorSpaces::Oklab);
    let green_oklab = Color::Green.convert(&ColorSpaces::Oklab);
    for i in 0..=255 {
        let t = i as f32 / 255.0;
        let want = Color::new_in(
            lerp(red_oklab.red(), green_oklab.red(), t),
            lerp(red_oklab.green(), green_oklab.green(), t),
            lerp(red_oklab.blue(), green_oklab.blue(), t),
            1.0,
            &ColorSpaces::Oklab,
        )
        .convert(&ColorSpaces::Srgb);
        assert_eq!(lerp_color(Color::Red, Color::Green, t).to_argb(), want.to_argb());
        let transparent_red = kompozable_block!({ Color::Red.copy(alpha = 0.0) });
        let along = kompozable_block!({ Color::Red.copy(alpha = lerp(1.0, 0.0, t)) });
        assert_eq!(lerp_color(Color::Red, transparent_red, t).to_argb(), along.to_argb());
    }
    // Within 0..1 (an easing's overshoot), in the stop's space.
    assert_eq!(lerp_color(Color::Red, Color::Blue, 1.5), lerp_color(Color::Red, Color::Blue, 1.0));
    assert_eq!(lerp_color(Color::Red, Color::Blue, -1.0), lerp_color(Color::Red, Color::Blue, 0.0));
    let p3 = lerp_color(Color::Red, Color::new_in(0.0, 0.0, 1.0, 1.0, &ColorSpaces::DisplayP3), 0.5);
    assert!(
        p3.color_space().equals(&ColorSpaces::DisplayP3)
            && lerp_color(Color::Black, Color::White, 0.5).to_argb() == 0xFF636363
    );
}

#[test]
fn colors_animate_in_oklab_as_composes_vector_converter() {
    // Black to white: Oklab's L along (0.5: a linear 0.125, 0x63 in sRGB), not sRGB's numbers (0x80).
    assert_eq!(animated_color(Color::Black, Color::White, 0.5).to_argb(), 0xFF636363);
    // Past the target, as a spring overshoots: L kept within 0..1, a and b within -0.5..0.5, alpha within 0..1.
    let over = animated_color(Color::Black, Color::White, 1.5);
    let under = animated_color(Color::White, Color::Black, 1.5);
    assert!(over.to_argb() == 0xFFFFFFFF && under.to_argb() == 0xFF000000);
    let red_oklab = Color::Red.convert(&ColorSpaces::Oklab);
    let blue_oklab = Color::Blue.convert(&ColorSpaces::Oklab);
    // (a and b past 0.5 three times as far)
    let past = Color::new_in(lerp(blue_oklab.red(), red_oklab.red(), 3.0), 0.5, 0.5, 1.0, &ColorSpaces::Oklab);
    assert!(
        lerp(blue_oklab.green(), red_oklab.green(), 3.0) > 0.5 && lerp(blue_oklab.blue(), red_oklab.blue(), 3.0) > 0.5
    );
    assert_eq!(animated_color(Color::Blue, Color::Red, 3.0), past.convert(&ColorSpaces::Srgb));
    let transparent_red = kompozable_block!({ Color::Red.copy(alpha = 0.0) });
    assert_eq!(animated_color(transparent_red, Color::Red, 2.0).alpha(), 1.0);
    assert_eq!(animated_color(Color::Red, transparent_red, 2.0).alpha(), 0.0);
    let to_p3 = animated_color(Color::Red, Color::new_in(0.0, 0.0, 1.0, 1.0, &ColorSpaces::DisplayP3), 0.25);
    assert!(to_p3.color_space().equals(&ColorSpaces::DisplayP3));
}

// ---- HCT, palettes and a picture's colors (Kompoz's tests) --------------------------------------------------------

#[test]
fn hct_matches_material_color_utilities() {
    // Material Color Utilities' values for the primaries.
    for (rgb, hue, chroma, tone) in
        [(0xFF0000, 27.408, 113.358, 53.233), (0x00FF00, 142.139, 108.410, 87.737), (0x0000FF, 282.788, 87.230, 32.302)]
    {
        let h = Hct::from_color(Color::hex(rgb));
        assert!(near64(h.hue(), hue, 0.01) && near64(h.chroma(), chroma, 0.01) && near64(h.tone(), tone, 0.01));
        // Back to itself, through its hue, chroma and tone.
        let (back, was) = (Color::from_argb(cam_to_color(h.hue(), h.chroma(), h.tone())), Color::hex(rgb));
        assert!(near(back.red(), was.red(), 1.5 / 255.0) && near(back.green(), was.green(), 1.5 / 255.0));
        assert!(near(back.blue(), was.blue(), 1.5 / 255.0));
        assert_eq!(Hct::from(h.hue(), h.chroma(), h.tone()).to_color(), was); // (HctSolver: exactly)
    }
    assert!(near64(Hct::from_color(Color::hex(0x000000)).tone(), 0.0, 0.5));
    assert!(near64(Hct::from_color(Color::hex(0xFFFFFF)).tone(), 100.0, 0.01));
    // Asked for more chroma than sRGB has at a tone: that hue and tone, as much chroma as there is.
    for tone in [10.0, 40.0, 50.0, 90.0] {
        let got = Hct::from_int(cam_to_color(282.8, 200.0, tone));
        assert!(near64(got.tone(), tone, 0.5) && near64(got.hue(), 282.8, 3.0) && got.chroma() < 200.0);
        let solved = Hct::from(282.8, 200.0, tone);
        assert!(near64(solved.tone(), tone, 0.5) && near64(solved.hue(), 282.8, 3.0) && solved.chroma() < 200.0);
    }
    // In gamut: what was asked for.
    let mid = Hct::from_int(cam_to_color(120.0, 30.0, 60.0));
    assert!(near64(mid.hue(), 120.0, 1.5) && near64(mid.chroma(), 30.0, 1.5) && near64(mid.tone(), 60.0, 0.5));
    // Under 1 of chroma: grey of that tone.
    for chroma in [0.0, 0.9] {
        let grey = Color::from_argb(cam_to_color(0.0, chroma, 50.0));
        assert!(
            grey.red() == grey.green()
                && grey.green() == grey.blue()
                && near64(Hct::from_color(grey).tone(), 50.0, 0.5)
        );
    }
}

#[test]
fn tonal_palette_from_a_seed_is_material_yous_tonal_spot() {
    let seed = Color::hex(0xB7CBC5); // a desktop's pale teal accent
    let hue = Hct::from_color(seed).hue();
    let p = DynamicTonalPalette::from_seed(seed);
    let check = |tones: &[Color; 13], h: f64, chroma: f64| {
        for (i, &color) in tones.iter().enumerate() {
            let got = Hct::from_color(color);
            let tone = DynamicTonalPalette::TONES[i] as f64;
            assert!(near64(got.tone(), tone, 0.6));
            // As much of that chroma as sRGB has at that tone (a dark teal can't have 36).
            let most = Hct::from_int(cam_to_color(h, 200.0, tone)).chroma();
            if (20.0..=90.0).contains(&tone) {
                assert!(near64(got.hue(), h, 3.0) && near64(got.chroma(), chroma.min(most), 2.0), "{tone}");
            }
        }
    };
    check(&p.accent1, hue, 36.0);
    check(&p.accent2, hue, 16.0);
    check(&p.accent3, (hue + 60.0) % 360.0, 24.0);
    check(&p.neutral1, hue, 6.0);
    check(&p.neutral2, hue, 8.0);
    assert!(p.accent1[0] == Color::hex(0xFFFFFF) && p.accent1[12] == Color::hex(0x000000));
    // As Compose's dynamicLightColorScheme takes it (Android's): primary at tone 40, its container at 90; the
    // tones the system has not (98) at the nearest one's lightness.
    assert!(palette_tone(&p.accent1, 40.0) == p.accent1[8] && palette_tone(&p.accent1, 90.0) == p.accent1[3]);
    let surface = palette_tone(&p.neutral1, 98.0);
    assert!(surface == palette_tone(&p.neutral1, 98.0) && near64(Hct::from_color(surface).tone(), 98.0, 0.5));
    assert!(near64(Hct::from_color(palette_tone(&p.accent1, 87.0)).tone(), 87.0, 0.5));
}

/// The grey of a tone (CIELAB lightness), in floats.
fn gray_tone(tone: f32) -> Color {
    let fy = (tone + 16.0) / 116.0;
    let y = if fy > 6.0 / 29.0 { fy * fy * fy } else { 3.0 * (6.0 / 29.0) * (6.0 / 29.0) * (fy - 4.0 / 29.0) };
    let v = if y <= 0.0031308 { y * 12.92 } else { 1.055 * y.powf(1.0 / 2.4) - 0.055 };
    Color::new(v, v, v, 1.0)
}

#[test]
fn palette_tones_the_system_does_not_give_are_made_at_their_lightness() {
    // Distinct colors, so which tone is taken shows; the neutrals grey.
    let tones = DynamicTonalPalette::TONES.map(|t| t as f32);
    let p = DynamicTonalPalette {
        accent1: tones.map(|t| Color::new(t / 100.0, 0.2, 0.4, 1.0)),
        accent2: tones.map(|t| Color::new(0.3, t / 100.0, 0.1, 1.0)),
        accent3: tones.map(|t| Color::new(0.5, 0.1, t / 100.0, 1.0)),
        neutral1: tones.map(gray_tone),
        neutral2: tones.map(gray_tone),
    };
    // The 13 as they are (Compose's mapping: primary 40, on primary 100, its container 90, …).
    assert!(palette_tone(&p.accent1, 40.0) == p.accent1[8] && palette_tone(&p.accent1, 100.0) == p.accent1[0]);
    assert!(palette_tone(&p.accent1, 90.0) == p.accent1[3] && palette_tone(&p.accent1, 10.0) == p.accent1[11]);
    assert!(palette_tone(&p.accent1, 80.0) == p.accent1[4] && palette_tone(&p.accent3, 40.0) == p.accent3[8]);
    // The others at that lightness: surface 98 between 100 and 95, the dark surface (6) between 10 and 0, still
    // grey.
    let between = |c: Color, lighter: Color, darker: Color| c.red() < lighter.red() && c.red() > darker.red();
    let (surface, dark_surface) = (palette_tone(&p.neutral1, 98.0), palette_tone(&p.neutral1, 6.0));
    assert!(
        between(surface, p.neutral1[0], p.neutral1[2])
            && near(surface.red() * 255.0, gray_tone(98.0).red() * 255.0, 0.5)
    );
    assert!(between(dark_surface, p.neutral1[11], p.neutral1[12]));
    assert!(near(dark_surface.green() * 255.0, dark_surface.blue() * 255.0, 0.5));
    assert!(between(palette_tone(&p.neutral1, 94.0), p.neutral1[2], p.neutral1[3]));
    assert!(near(palette_tone(&p.neutral1, 87.0).red() * 255.0, gray_tone(87.0).red() * 255.0, 0.5));
}

/// A picture of `colors` in a row: each (0xRRGGBB, how many pixels), opaque; the last group transparent if asked.
fn pixels_of(colors: &[(u32, usize)], transparent_last: bool) -> Vec<u32> {
    let mut pixels = Vec::new();
    for (k, &(c, count)) in colors.iter().enumerate() {
        let alpha = if transparent_last && k == colors.len() - 1 { 0 } else { 0xff000000 };
        pixels.extend(std::iter::repeat_n(alpha | (c & 0xffffff), count));
    }
    pixels
}

fn hex_of(argb: u32) -> u32 {
    argb & 0xffffff
}

#[test]
fn quantizer_finds_a_pictures_colors_and_how_much_of_each() {
    let colors_of = |pixels: Vec<u32>, max_colors: i32| -> Vec<(u32, u32)> {
        QuantizerCelebi::quantize(&pixels, max_colors).color_to_count.into_iter().map(|(c, n)| (hex_of(c), n)).collect()
    };
    assert_eq!(colors_of(pixels_of(&[(0xFF0000, 1)], false), 128), [(0xFF0000, 1)]);
    assert_eq!(colors_of(pixels_of(&[(0x141216, 1)], false), 128), [(0x141216, 1)]);
    assert_eq!(colors_of(pixels_of(&[(0x0000FF, 5)], false), 128), [(0x0000FF, 5)]);
    // Three colors, the most first.
    assert_eq!(
        colors_of(pixels_of(&[(0x00FF00, 30), (0x0000FF, 10), (0xFF0000, 60)], false), 128),
        [(0xFF0000, 60), (0x00FF00, 30), (0x0000FF, 10)]
    );
    // Pixels that aren't opaque don't count; nothing at all gives nothing.
    assert_eq!(colors_of(pixels_of(&[(0xFF0000, 2), (0x00FF00, 7)], true), 128), [(0xFF0000, 2)]);
    assert!(colors_of(Vec::new(), 128).is_empty() && colors_of(pixels_of(&[(0xFF0000, 1)], false), 0).is_empty());
    assert!(colors_of(pixels_of(&[(0xFF0000, 3)], true), 128).is_empty());
    // One color asked for: the mean of them all, every pixel in it.
    assert_eq!(colors_of(pixels_of(&[(0xFF0000, 1), (0xFF0010, 1)], false), 1), [(0xFF0008, 2)]);
    // Close shades of one color and a far one: two colors, the shades together.
    let two = colors_of(pixels_of(&[(0x2060C0, 10), (0x2262C2, 10), (0x1E5EBE, 10), (0xE0A020, 5)], false), 2);
    assert!(two.len() == 2 && two[0].1 == 30 && two[1].1 == 5 && two[1].0 == 0xE0A020);
    assert!(((two[0].0 >> 16) as i32 - 0x20).abs() <= 2 && ((two[0].0 & 0xFF) as i32 - 0xC0).abs() <= 2);
    // Colors that differ only in blue are told apart (each channel is an axis of Wu's cube).
    let blues = colors_of(pixels_of(&[(0x000000, 10), (0x0000FF, 10)], false), 2);
    assert!(blues.len() == 2 && blues[0].1 == 10 && blues[1].1 == 10);
}

#[test]
fn seed_colors_are_picked_as_androids_system_ui_picks_them() {
    let seeds = |colors: &[u32], desired: usize| -> Vec<u32> {
        let input: Vec<(u32, u32)> = colors.iter().map(|&c| (0xff000000 | c, 1)).collect();
        seed_colors(&input, desired).into_iter().map(hex_of).collect()
    };
    assert_eq!(seeds(&[0x000000, 0xFFFFFF, 0x0000FF], 4), [0x0000FF]); // chroma first: greys left out
    assert_eq!(seeds(&[0xFF0000, 0x00FF00, 0x0000FF], 4), [0xFF0000, 0x00FF00, 0x0000FF]); // as much of each: by chroma
    assert_eq!(seeds(&[0x000000], 4), [0x1B6EF3]); // none fit: Google blue
    assert_eq!(seeds(&[], 4), [0x1B6EF3]);
    assert_eq!(seeds(&[0x008772, 0x318477], 4), [0x008772]); // hues this close: one of them
    assert_eq!(seeds(&[0x008772, 0x008587, 0x007EBC], 2), [0x007EBC, 0x008772]); // as far apart as two can be
    // A color that covers 1% of the picture or less, around its hue, isn't a seed.
    let mut mostly_grey = vec![(0xff808080, 999), (0xffff0000, 1)];
    assert_eq!(seed_colors(&mostly_grey, 4), [GOOGLE_BLUE]);
    mostly_grey[0].1 = 98;
    assert_eq!(hex_of(seed_colors(&mostly_grey, 4)[0]), 0xFF0000);
    let of_hct = |hue: f64, chroma: f64, tone: f64, n: u32| (cam_to_color(hue, chroma, tone), n);
    let hue_of = |c: u32| Hct::from_int(c).hue();
    // Chroma under 48 counts against a color less than chroma over it counts for one (0.1 vs 0.3 a unit): 56% of
    // chroma 40 beats 44% of chroma 70.
    let picked = seed_colors(&[of_hct(120.0, 40.0, 60.0, 56), of_hct(300.0, 70.0, 50.0, 44)], 1);
    assert!(picked.len() == 1 && (hue_of(picked[0]) - 120.0).abs() < 3.0);
    // Picked as far apart as they can be: of hues 20, 40 and 140, two are 20 and 140.
    let spread =
        seed_colors(&[of_hct(20.0, 60.0, 50.0, 1), of_hct(40.0, 58.0, 50.0, 1), of_hct(140.0, 56.0, 50.0, 1)], 2);
    assert!(spread.len() == 2 && (hue_of(spread[0]) - 20.0).abs() < 3.0 && (hue_of(spread[1]) - 140.0).abs() < 3.0);
    // Greys don't count for their hue: a colorful 1 in 201 of the picture is too little, though the near grey of
    // the same hue is the rest.
    assert_eq!(seed_colors(&[of_hct(30.0, 40.0, 50.0, 1), of_hct(30.0, 3.0, 50.0, 200)], 4), [GOOGLE_BLUE]);
    assert!(
        (hue_of(seed_colors(&[of_hct(30.0, 40.0, 50.0, 3), of_hct(30.0, 3.0, 50.0, 200)], 4)[0]) - 30.0).abs() < 3.0
    );
    // A color's share counts what is within 15° of its hue, both ends in: alone, 1 in 101 is too little, but with
    // a neighbor exactly 15° off it is enough to be the second seed.
    let rare = of_hct(100.0, 60.0, 50.0, 1);
    let rare_hue = hue_of(rare.0).round() as i64;
    for off in [-15i64, 15] {
        let mut neighbor = None;
        let mut h = (rare_hue + off) as f64 - 1.5;
        // (Its 8-bit color's hue rounds right.)
        while h < (rare_hue + off) as f64 + 1.5 && neighbor.is_none() {
            let c = of_hct(h, 40.0, 50.0, 100);
            if hue_of(c.0).round() as i64 == rare_hue + off && (hue_of(c.0) - hue_of(rare.0)).abs() >= 15.0 {
                neighbor = Some(c);
            }
            h += 0.05;
        }
        let neighbor = neighbor.expect("a neighbor 15° off");
        assert_eq!(seed_colors(&[rare, neighbor], 2).len(), 2);
    }
    // Hue distance goes round the circle: 355° and 5° are 10° apart, too close for two seeds.
    assert_eq!(seed_colors(&[of_hct(355.0, 60.0, 50.0, 1), of_hct(5.0, 58.0, 50.0, 1)], 2).len(), 1);
}

#[test]
fn tonal_palette_from_a_picture_takes_its_best_seed() {
    // 1000 × 600: mostly a blue, a stripe of white; scaled down to about 112 × 112 first.
    let (w, h) = (1000, 600);
    let mut pixels = vec![0u32; w * h];
    for y in 0..h {
        for x in 0..w {
            pixels[y * w + x] = if y < h / 5 { 0xffffffff } else { 0xff1e88e5 };
        }
    }
    let p = DynamicTonalPalette::from_image(&pixels, w, h);
    let blue = DynamicTonalPalette::from_seed(Color::hex(0x1E88E5));
    assert!((Hct::from_color(p.accent1[8]).hue() - Hct::from_color(blue.accent1[8]).hue()).abs() < 1.0);
    assert_eq!(DynamicTonalPalette::from_image(&[], 0, 0), DynamicTonalPalette::from_seed(Color::hex(0x1B6EF3)));
    // Scaled down without filtering, as Android does: each pixel the one under its center, so stripes of red (even
    // columns) and blue (odd ones) halved are all blue; unscaled, red wins.
    let stripes: Vec<u32> = (0..224 * 224).map(|i| if (i % 224) % 2 == 0 { 0xffff0000 } else { 0xff0000ff }).collect();
    let striped = DynamicTonalPalette::from_image(&stripes, 224, 224);
    let all_blue = DynamicTonalPalette::from_seed(Color::hex(0x0000FF));
    assert!((Hct::from_color(striped.accent1[8]).hue() - Hct::from_color(all_blue.accent1[8]).hue()).abs() < 2.0);
    // A picture of greys only: Google blue's.
    let greys = pixels_of(&[(0x777777, 50), (0x222222, 50)], false);
    assert_eq!(DynamicTonalPalette::from_image(&greys, 100, 1), DynamicTonalPalette::from_seed(Color::hex(0x1B6EF3)));
}

#[test]
fn wallpaper_colors_pick_main_colors_and_hints_as_android() {
    // A bright picture, mostly white with some blue: dark text suits it, a dark theme not.
    let bright = pixels_of(&[(0xFFFFFF, 90), (0x1E88E5, 10)], false);
    let colors = WallpaperColors::from_bitmap(&bright, 10, 10);
    assert_eq!(colors.color_hints(), WallpaperColors::HINT_FROM_BITMAP | WallpaperColors::HINT_SUPPORTS_DARK_TEXT);
    // Its colors, the most first; the main ones by chroma and share of the picture (greys too): the white, much of
    // it, before the blue.
    assert_eq!(
        colors.all_colors().iter().map(|&(c, n)| (hex_of(c), n)).collect::<Vec<_>>(),
        [(0xFFFFFF, 90), (0x1E88E5, 10)]
    );
    assert_eq!(colors.main_colors(), [Color::White, Color::hex(0x1E88E5)]);
    assert!(colors.primary_color() == Color::White && colors.tertiary_color().is_none());
    // Bright but with more than 5% too dark for dark text: no hint for it.
    let spotted = pixels_of(&[(0xFFFFFF, 90), (0x000000, 10)], false);
    assert_eq!(
        WallpaperColors::from_bitmap(&spotted, 10, 10).color_hints() & WallpaperColors::HINT_SUPPORTS_DARK_TEXT,
        0
    );
    // A dark picture: a dark theme suits it; dimmed, a bright one too.
    let dark = pixels_of(&[(0x202020, 100)], false);
    assert_ne!(
        WallpaperColors::from_bitmap(&dark, 10, 10).color_hints() & WallpaperColors::HINT_SUPPORTS_DARK_THEME,
        0
    );
    let dimmed = WallpaperColors::from_bitmap_dimmed(&bright, 10, 10, 0.8);
    assert_eq!(dimmed.color_hints(), WallpaperColors::HINT_FROM_BITMAP | WallpaperColors::HINT_SUPPORTS_DARK_THEME);
    // Main colors at least 15° apart: of two blues 5° apart, one.
    let blues =
        WallpaperColors::new(vec![(cam_to_color(250.0, 50.0, 50.0), 10), (cam_to_color(255.0, 50.0, 50.0), 10)], 0);
    assert_eq!(blues.main_colors().len(), 1);
    // From colors: a dark primary hints a dark theme; a tertiary needs a secondary.
    let from = WallpaperColors::from_colors(Color::hex(0x101030), Some(Color::Red), None);
    assert!(
        from.color_hints() == WallpaperColors::HINT_SUPPORTS_DARK_THEME && from.secondary_color() == Some(Color::Red)
    );
    assert!(panics(|| {
        WallpaperColors::from_colors(Color::Red, None, Some(Color::Blue));
    }));
}

// ---- Material Color Utilities: its numbers (its reference implementation's) -------------------------------------

#[test]
fn hct_solver_and_cam16_give_material_color_utilities_numbers() {
    for (argb, hue, chroma, tone) in [
        (0xFFFF0000u32, 27.40822513715874, 113.3578873937978, 53.23288178584245),
        (0xFF00FF00, 142.1398935076457, 108.41006052028892, 87.73703347354422),
        (0xFF0000FF, 282.78817956187277, 87.23069368032536, 32.302586667249486),
        (0xFF6750A4, 298.980997210704, 47.8565263749703, 40.08324408746242),
        (0xFF1B6EF3, 268.98680143081174, 69.62500993972414, 49.320906686938244),
    ] {
        let h = Hct::from_int(argb);
        assert!(near64(h.hue(), hue, 1e-12) && near64(h.chroma(), chroma, 1e-12) && near64(h.tone(), tone, 1e-12));
        assert_eq!(h.to_int(), argb);
    }
    for (hue, chroma, tone, argb) in [
        (282.8, 200.0, 40.0, 0xFF343DFFu32),
        (120.0, 30.0, 60.0, 0xFF8C965A),
        (27.0, 113.0, 53.0, 0xFFFE0005),
        (300.0, 50.0, 10.0, 0xFF23005C),
        (90.0, 80.0, 95.0, 0xFFFFEFCF),
        (0.0, 0.9, 50.0, 0xFF7A7676),
        (210.0, 16.0, 99.0, 0xFFF6FEFF),
    ] {
        assert_eq!(Hct::from(hue, chroma, tone).to_int(), argb, "{hue} {chroma} {tone}");
    }
    // Setting a part keeps the others (as far as sRGB lets it).
    let mut h = Hct::from_int(0xFF6750A4);
    h.set_tone(90.0);
    assert!(near64(h.tone(), 90.0, 0.5) && near64(h.hue(), 298.98, 2.0));
    // CAM16: a color to itself, and its UCS distance.
    let cam = Cam16::from_int(0xFF6750A4);
    assert_eq!(Cam16::from_jch(cam.j(), cam.chroma(), cam.hue()).to_int(), 0xFF6750A4);
    assert_eq!(Cam16::from_ucs(cam.jstar(), cam.astar(), cam.bstar()).to_int(), 0xFF6750A4);
    assert!(cam.distance(&cam) == 0.0 && cam.distance(&Cam16::from_int(0xFF000000)) > 10.0);
    // The color that looks in sRGB's conditions as this one does on another background: on a dark one, darker;
    // on a light one, lighter (Material Color Utilities' numbers).
    let seen_on = |background: f64| {
        Hct::from_int(0xFF6750A4)
            .in_viewing_conditions(&ViewingConditions::default_with_background_lstar(background))
            .to_int()
    };
    assert!(seen_on(10.0) == 0xFF534674 && seen_on(90.0) == 0xFF7C62C1);
}

#[test]
fn tonal_palettes_are_material_color_utilities() {
    let tones = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 95, 99, 100];
    for (hue, chroma, key, want) in [
        (
            270.0,
            36.0,
            0xFF6176ACu32,
            [
                0xFF000000u32,
                0xFF001847,
                0xFF172E60,
                0xFF304578,
                0xFF485D92,
                0xFF6176AC,
                0xFF7B8FC8,
                0xFF95AAE4,
                0xFFB1C5FF,
                0xFFDAE2FF,
                0xFFEEF0FF,
                0xFFFEFBFF,
                0xFFFFFFFF,
            ],
        ),
        (
            30.0,
            80.0,
            0xFFD93F23,
            [
                0xFF000000, 0xFF3E0400, 0xFF650B00, 0xFF8E1400, 0xFFB6250A, 0xFFD93F23, 0xFFFD5839, 0xFFFF8A73,
                0xFFFFB4A5, 0xFFFFDAD3, 0xFFFFEDE9, 0xFFFFFBFF, 0xFFFFFFFF,
            ],
        ),
        (
            150.0,
            6.0,
            0xFF747871,
            [
                0xFF000000, 0xFF181D18, 0xFF2D322C, 0xFF434842, 0xFF5B6059, 0xFF747871, 0xFF8D928B, 0xFFA8ADA5,
                0xFFC4C8C0, 0xFFE0E4DB, 0xFFEEF2E9, 0xFFFAFEF5, 0xFFFFFFFF,
            ],
        ),
    ] {
        let p = TonalPalette::from_hue_and_chroma(hue, chroma);
        assert_eq!(p.key_color().to_int(), key);
        assert_eq!(tones.map(|t| p.tone(t)), want);
    }
    // A color's palette: its hue and chroma, itself the key color.
    let p = TonalPalette::from_int(0xFF6750A4);
    assert!(p.key_color().to_int() == 0xFF6750A4 && near64(p.hue(), 298.98, 0.01));
    // Yellows at 99: between 98 and 100.
    let yellow = TonalPalette::from_hue_and_chroma(110.0, 60.0);
    let (t98, t100) = (yellow.tone(98), yellow.tone(100));
    let average = |shift: u32| (((t98 >> shift & 0xff) + (t100 >> shift & 0xff)) as f32 / 2.0 + 0.5).floor() as u32;
    assert_eq!(yellow.tone(99), 0xff000000 | average(16) << 16 | average(8) << 8 | average(0));
    // CorePalette: a1 at least 48 of chroma, a3 turned 60°.
    let core = CorePalette::of(0xFF6750A4);
    assert!(
        near64(core.a1.chroma(), 48.0, 1e-9)
            && near64(core.a3.hue(), 358.98, 0.01)
            && near64(core.error.hue(), 25.0, 1e-9)
    );
    let content = CorePalette::content_of(0xFF6750A4);
    assert!(near64(content.a2.chroma(), content.a1.chroma() / 3.0, 1e-9) && content.n1.chroma() <= 4.0);
}

#[test]
fn contrast_dislike_score_and_temperature_are_material_color_utilities() {
    // Contrast: white on black is 21, a tone on itself 1; lighter and darker tones that make a ratio.
    assert!(
        near64(Contrast::ratio_of_tones(100.0, 0.0), 21.0, 1e-9)
            && near64(Contrast::ratio_of_tones(50.0, 50.0), 1.0, 1e-9)
    );
    let lighter = Contrast::lighter(40.0, 4.5);
    assert!(lighter > 40.0 && Contrast::ratio_of_tones(lighter, 40.0) >= 4.5 - 0.04);
    assert!(Contrast::lighter(90.0, 4.5) == -1.0 && Contrast::lighter_unsafe(90.0, 4.5) == 100.0);
    let darker = Contrast::darker(60.0, 3.0);
    assert!(darker < 60.0 && Contrast::ratio_of_tones(60.0, darker) >= 3.0 - 0.04);
    assert!(Contrast::darker(10.0, 7.0) == -1.0 && Contrast::darker_unsafe(10.0, 7.0) == 0.0);
    // Dislike: a dark yellow-green, fixed at tone 70.
    let bile = Hct::from(100.0, 40.0, 40.0);
    assert!(DislikeAnalyzer::is_disliked(&bile) && !DislikeAnalyzer::is_disliked(&Hct::from(100.0, 40.0, 70.0)));
    assert!(near64(DislikeAnalyzer::fix_if_disliked(bile).tone(), 70.0, 0.5));
    assert!(!DislikeAnalyzer::is_disliked(&Hct::from_int(0xFF6750A4)));
    // Score: Material Color Utilities' picks.
    assert_eq!(
        Score::score(&[(0xFFFF0000, 1), (0xFF00FF00, 1), (0xFF0000FF, 1)]),
        [0xFFFF0000, 0xFF00FF00, 0xFF0000FF]
    );
    assert_eq!(Score::score(&[(0xFF008772, 1), (0xFF318477, 1)]), [0xFF008772]);
    assert_eq!(Score::score(&[(0xFF000000, 1)]), [0xFF4285F4]);
    assert_eq!(
        Score::score(&[(0xFF4285F4, 10), (0xFFEA4335, 5), (0xFFFBBC04, 3), (0xFF34A853, 2), (0xFF808080, 50)]),
        [0xFFEA4335, 0xFF4285F4, 0xFFFBBC04, 0xFF34A853]
    );
    // Temperature: the complement and the analogous colors.
    let t = TemperatureCache::new(Hct::from_int(0xFF6750A4));
    assert_eq!(t.complement().to_int(), 0xFF765B00);
    let analogous: Vec<u32> = t.analogous_colors().iter().map(Hct::to_int).collect();
    assert_eq!(analogous, [0xFF006B61, 0xFF2A5EA7, 0xFF6750A4, 0xFF834790, 0xFF924274]);
    let t = TemperatureCache::new(Hct::from_int(0xFF1E88E5));
    assert_eq!(t.complement().to_int(), 0xFFCC6C00);
    let analogous: Vec<u32> = t.analogous_colors_with(5, 12).iter().map(Hct::to_int).collect();
    assert_eq!(analogous, [0xFF329844, 0xFF00977B, 0xFF1E88E5, 0xFF8A74E7, 0xFFB267CE]);
    assert!(
        TemperatureCache::raw_temperature(&Hct::from_int(0xFFFF8000))
            > TemperatureCache::raw_temperature(&Hct::from_int(0xFF0080FF))
    );
}

/// Material Color Utilities' schemes (its reference implementation's colors): a seed, a variant, a spec, dark or
/// not and a contrast level, then each role's color in MaterialDynamicColors' order ("-": none in that spec).
const SCHEMES: &[&str] = &[
    "FF6750A4 TonalSpot 2021 0 0.0",
    "FFFDF7FF FF1D1B20 FFFDF7FF FFDED8E0 FFFDF7FF FFFFFFFF FFF8F2FA FFF2ECF4 FFECE6EE",
    "FFE6E0E9 FF1D1B20 FFE7E0EB FF49454E FF7A757F FFCAC4CF FF322F35 FFF5EFF7 FF000000",
    "FF000000 FF65558F FF65558F FF594982 FFFFFFFF FFE9DDFF FF4D3D75 FFCFBDFE FFE9DDFF",
    "FFCFBDFE FF201047 FF4D3D75 FF625B71 FF564F64 FFFFFFFF FFE8DEF8 FF4A4458 FFE8DEF8",
    "FFCBC2DB FF1E192B FF4A4458 FF7E5260 FF704654 FFFFFFFF FFFFD9E3 FF633B48 FFFFD9E3",
    "FFEFB8C8 FF31101D FF633B48 FFBA1A1A FFA80710 FFFFFFFF FFFFDAD6 FF93000A FF7E6EA9",
    "FF7B748A FF996A79 FF7A767D FF7A757F FFDE3730",
    "FF6750A4 TonalSpot 2021 1 0.5",
    "FF141218 FFE6E0E9 FF141218 FF141218 FF46434A FF08070B FF1F1D22 FF29272D FF343138",
    "FF3F3C43 FFFFFFFF FF49454E FFE0DAE5 FFB5B0BB FF938E99 FFE6E0E9 FF2B292F FF000000",
    "FF000000 FFCFBDFE FFE3D6FF FFCAB7F9 FF2B1B52 FF9887C5 FF000000 FF4E3F77 FFE9DDFF",
    "FFCFBDFE FF16033D FF3C2D63 FFE2D8F2 FFE8DEF8 FF282336 FF958DA4 FF000000 FFE8DEF8",
    "FFCBC2DB FF130E20 FF393347 FFFFD0DD FFFEC5D6 FF3D1A27 FFB58392 FF000000 FFFFD9E3",
    "FFEFB8C8 FF240613 FF502B38 FFFFD2CC FFFFACA2 FF540003 FFFF5449 FF000000 FF7E6EA9",
    "FF7B748A FF996A79 FF7A767D FF7A757F FFDE3730",
    "FF6750A4 Monochrome 2021 0 0.0",
    "FFF9F9F9 FF1B1B1B FFF9F9F9 FFDADADA FFF9F9F9 FFFFFFFF FFF3F3F3 FFEEEEEE FFE8E8E8",
    "FFE2E2E2 FF1B1B1B FFE2E2E2 FF474747 FF777777 FFC6C6C6 FF303030 FFF1F1F1 FF000000",
    "FF000000 FF5E5E5E FF000000 FF000000 FFE2E2E2 FF3B3B3B FFFFFFFF FFC6C6C6 FF5E5E5E",
    "FF474747 FFFFFFFF FFE2E2E2 FF5E5E5E FF525252 FFFFFFFF FFD4D4D4 FF1B1B1B FFC6C6C6",
    "FFABABAB FF1B1B1B FF3B3B3B FF3B3B3B FF303030 FFE2E2E2 FF747474 FFFFFFFF FF5E5E5E",
    "FF474747 FFFFFFFF FFE2E2E2 FFBA1A1A FFA80710 FFFFFFFF FFFFDAD6 FF410002 FF777777",
    "FF777777 FF777777 FF777777 FF777777 FFDE3730",
    "FF6750A4 Rainbow 2021 1 -1.0",
    "FF131313 FF626262 FF131313 FF131313 FF393939 FF0E0E0E FF1B1B1B FF1F1F1F FF2A2A2A",
    "FF353535 FFA3A3A3 FF474747 FF838383 FF545454 FF3A3A3A FFE2E2E2 FF646464 FF000000",
    "FF000000 FFCFBCFF FF9A83DB FF9B83DB FF31166B FF42297D FF8E76CD FF8A72C9 FF594295",
    "FF42297D FFCDBAFF FFAA92EC FF958DA4 FF958DA5 FF2C273A FF3D374B FF888197 FF544E63",
    "FF3D374B FFC9C0D9 FFA49CB4 FFB58392 FFB58393 FF421E2B FF552E3C FFA77786 FF6F4553",
    "FF552E3C FFECB5C5 FFC692A2 FFFF5449 FFFF554A FF5C0003 FF7B0007 FFF0443B FF8069BF",
    "FF7B748A FF996A79 FF777777 FF777777 FFDE3730",
    "FF1E88E5 Vibrant 2025 1 0.0",
    "FF000D28 FFDCE5FF FF000D28 FF000D28 FF042A5F FF000000 FF001232 FF00183E FF001E49",
    "FF012455 FFDCE5FF FF012455 FF99ABD4 FF64759C FF36486C FFF9F9FF FF43557A FF000000",
    "FF000000 FF70B1FF FF70B1FF FF47A1FF FF002F58 FF4FA4FF FF002444 FF0061A9 FF4FA4FF",
    "FF1696FF FF000000 FF002D54 FF699CFE FF699CFE FF001E4A FF0351AF FFDFE7FF FFBFD1FF",
    "FFA9C4FF FF003271 FF004DA8 FFE5ACFF FFDE98FF FF5C1C7D FFDE98FF FF510D72 FFDE98FF",
    "FFCF8BF0 FF300048 FF5B1A7C FFFF716C FFD7383B FF490006 FF9F0519 FFFFA8A3 FF0093FD",
    "FF3E75D4 FF9D5CBE FF66779E FF5D77AC FFDB3A3D",
    "FF1E88E5 Expressive 2025 0 -1.0",
    "FFF8F9FF FF153454 FFF8F9FF FFC4DCFF FFF8F9FF FFFFFFFF FFEFF4FF FFE6EEFF FFDCE9FF",
    "FFD3E4FF FF153454 FFD3E4FF FF456184 FF617CA1 FF98B4DB FF020F1F FF909EB4 FF000000",
    "FF000000 FF1461A5 FF1461A5 FF246AAF FFF8F8FF FF97C4FF FF003E6F FF5693DB FF97C4FF",
    "FF7CB6FF FF00294C FF00477E FF436750 FF4C7059 FFE7FFEC FFD2FBDE FF3E624C FFD2FBDE",
    "FFC4ECD0 FF2C4F3A FF486C56 FF006E39 FF137741 FFE8FFE9 FF95F1AE FF005B2E FF95F1AE",
    "FF87E2A1 FF004622 FF006635 FFAC3434 FFB83C3B FFFFF7F6 FFF56965 FF65000B FF397AC0",
    "FF5A7F67 FF2A874F FF6A788D FF4879B3 FFCC4B49",
    "FF1E88E5 Neutral 2025 0 0.5",
    "FFFCF8F9 FF232426 FFFCF8F9 FFDAD9DD FFFCF8F9 FFFFFFFF FFF6F3F4 FFEFEDEF FFE9E7EA",
    "FFE3E2E4 FF232426 FFE3E2E4 FF424345 FF5F5F61 FF7A7A7D FF0E0E0F FFC6C4C5 FF000000",
    "FF000000 FF3C434F FF3C434F FF313843 FFD3DAE9 FF6D7582 FFFFFFFF FFE4EBFA FF6D7582",
    "FF616975 FFFFFFFF FFFFFFFF FF404348 FF35383D FFD8D9E0 FF72747A FFFFFFFF FF72747A",
    "FF66686E FFFFFFFF FFFFFFFF FF3E415B FF33364F FFD6D7F7 FF70738F FFFFFFFF FF70738F",
    "FF646682 FFFFFFFF FFFFFFFF FF7A2524 FF4E0309 FFFFCECA FFBB5551 FFFFFFFF FF707784",
    "FF75777D FF737591 FF787778 FF777779 FFBE5753",
    "FF1E88E5 FruitSalad 2021 0 0.0",
    "FFF8F9FF FF161C24 FFF8F9FF FFD5DAE6 FFF8F9FF FFFFFFFF FFEFF4FF FFE9EEFA FFE3E8F4",
    "FFDDE3EE FF161C24 FFD7E3F8 FF3C4858 FF6C788A FFBBC7DB FF2B313A FFECF1FD FF000000",
    "FF000000 FF006971 FF006971 FF005C63 FFFFFFFF FF88F3FF FF004F56 FF4DD8E7 FF88F3FF",
    "FF4DD8E7 FF001F23 FF004F56 FF006971 FF005C63 FFFFFFFF FF9DF0FA FF004F56 FF9DF0FA",
    "FF81D3DD FF001F23 FF004F56 FF39608F FF2B5482 FFFFFFFF FFD3E4FF FF1D4875 FFD3E4FF",
    "FFA3C9FE FF001C38 FF1D4875 FFBA1A1A FFA80710 FFFFFFFF FFFFDAD6 FF93000A FF0EB7C5",
    "FF27828B FF5379A9 FF717782 FF6C788A FFDE3730",
    "FFB7CBC5 TonalSpot 2025 1 0.0",
    "FF0A0F0E FFDDE8E4 FF0A0F0E FF0A0F0E FF242E2C FF000000 FF0E1513 FF131B19 FF19211F",
    "FF1E2825 FFDDE8E4 FF1E2825 FFA2ADAA FF6D7874 FF404A47 FFF6FAF7 FF515654 FF000000",
    "FF000000 FF9DD1C5 FF9DD1C5 FF8FC3B7 FF12483F FF285B51 FFB9EEE1 FF36685E FFB8EDE0",
    "FFAADFD2 FF11473E FF32645A FFB1CCC5 FFA3BEB7 FF2C453F FF28403B FFAAC5BE FFCDE8E1",
    "FFBFDAD3 FF2B443F FF47605A FFD4F3FF FFB4E8FB FF2A5F6F FFB4E8FB FF205667 FFB4E8FB",
    "FFA6D9ED FF034453 FF2C6071 FFFA746F FFC54D4A FF490006 FF871F21 FFFF9993 FF4E8076",
    "FF637C76 FF4B7E8F FF737876 FF6E7976 FFC84F4C",
    "FFB7CBC5 Content 2021 0 0.0",
    "FFFBF9F8 FF1B1C1B FFFBF9F8 FFDBDAD8 FFFBF9F8 FFFFFFFF FFF5F3F2 FFEFEDEC FFE9E8E6",
    "FFE4E2E1 FF1B1C1B FFDEE4E1 FF424846 FF727876 FFC2C8C5 FF303030 FFF2F0EF FF000000",
    "FF000000 FF50625D FF50625D FF445652 FFFFFFFF FFB7CBC5 FF445652 FFB7CBC5 FFD3E7E1",
    "FFB7CBC5 FF0D1F1B FF384A46 FF59605D FF4D5452 FFFFFFFF FFDAE1DE FF5D6462 FFDDE4E1",
    "FFC1C8C5 FF161D1B FF414846 FF5D5D6C FF515160 FFFFFFFF FFC7C5D7 FF525160 FFE3E0F3",
    "FFC7C5D7 FF1A1A27 FF464554 FFBA1A1A FFA80710 FFFFFFFF FFFFDAD6 FF93000A FF687B76",
    "FF727976 FFC7C5D7 FF777776 FF727876 FFDE3730",
    "FFB7CBC5 Fidelity 2021 1 0.5",
    "FF131413 FFE4E2E1 FF131413 FF131413 FF444444 FF070807 FF1D1E1D FF272827 FF323332",
    "FF3D3E3D FFFFFFFF FF424846 FFD8DEDB FFADB3B0 FF8C928F FFE4E2E1 FF292A2A FF000000",
    "FF000000 FFB7CBC5 FFD3E7E1 FFE1F5EF FF1C2D2A FFB7CBC5 FF283A36 FF3A4C47 FFD3E7E1",
    "FFB7CBC5 FF031411 FF283A35 FFD7DEDB FFDDE4E1 FF202725 FF8B928F FF000000 FFDDE4E1",
    "FFC1C8C5 FF0C1211 FF313836 FFFEDBD3 FFF0CDC6 FF3A2520 FFE1BFB8 FF47312C FFFEDBD3",
    "FFE1BFB8 FF1E0D09 FF47312C FFFFD2CC FFFFACA2 FF540003 FFFF5449 FF000000 FF687B76",
    "FF727976 FFE1BFB8 FF777776 FF727876 FFDE3730",
];

#[test]
fn dynamic_schemes_give_material_color_utilities_colors() {
    let roles = MaterialDynamicColors.all_colors();
    let hex = |s: &str| u32::from_str_radix(s, 16).unwrap();
    let mut lines = SCHEMES.iter().peekable();
    let mut schemes = 0;
    while let Some(head) = lines.next() {
        let w: Vec<&str> = head.split(' ').collect();
        let variant = match w[1] {
            "TonalSpot" => Variant::TonalSpot,
            "Vibrant" => Variant::Vibrant,
            "Expressive" => Variant::Expressive,
            "Content" => Variant::Content,
            "Fidelity" => Variant::Fidelity,
            "Monochrome" => Variant::Monochrome,
            "Neutral" => Variant::Neutral,
            "Rainbow" => Variant::Rainbow,
            _ => Variant::FruitSalad,
        };
        let spec = if w[2] == "2025" { SpecVersion::Spec2025 } else { SpecVersion::Spec2021 };
        let scheme = DynamicScheme::new(
            Hct::from_int(hex(w[0])),
            variant,
            w[3] == "1",
            w[4].parse().unwrap(),
            Platform::Phone,
            spec,
        );
        let mut values = Vec::new();
        while let Some(line) = lines.peek() {
            if line.contains('.') {
                break; // (the next scheme's head: its contrast level)
            }
            values.extend(lines.next().unwrap().split(' '));
        }
        assert_eq!(values.len(), roles.len());
        for (role, value) in roles.iter().zip(values) {
            if value != "-" {
                assert_eq!(format!("{:08X}", role.get_argb(&scheme)), value, "{head}: {}", role.name);
            }
        }
        schemes += 1;
    }
    assert_eq!(schemes, 11);
}

#[test]
fn schemes_answer_roles_as_colors() {
    let seed = Hct::from_int(0xFF6750A4);
    let light = SchemeTonalSpot::new(seed, false, 0.0);
    assert!(light.variant == Variant::TonalSpot && light.spec_version == SpecVersion::Spec2021 && !light.is_dark);
    assert_eq!(light.primary(), Color::from_argb(0xFF65558F));
    assert_eq!(light.primary(), light.get_color(&MaterialDynamicColors.primary()));
    assert_eq!(light.on_primary(), Color::White);
    let dark = DynamicScheme::from_scheme(&light, true, 0.0);
    assert!(dark.is_dark && dark.primary_palette == light.primary_palette && dark.primary() != light.primary());
    // A spec other than 2021 only for tonal spot, neutral, vibrant and expressive.
    let content = SchemeContent::with_spec(seed, false, 0.0, SpecVersion::Spec2025, Platform::Phone);
    assert_eq!(content.spec_version, SpecVersion::Spec2021);
    let expressive = SchemeExpressive::with_spec(seed, false, 0.0, SpecVersion::Spec2025, Platform::Watch);
    assert!(expressive.spec_version == SpecVersion::Spec2025 && expressive.platform == Platform::Watch);
    assert_eq!(expressive.surface(), Color::Black); // (a watch's surface: tone 0)
    // Each variant makes its palettes: monochrome's are grey, fidelity's primary the source's chroma.
    assert_eq!(SchemeMonochrome::new(seed, false, 0.0).primary_palette.chroma(), 0.0);
    assert!(near64(SchemeFidelity::new(seed, false, 0.0).primary_palette.chroma(), seed.chroma(), 1e-9));
    for scheme in [
        SchemeVibrant::new(seed, true, 1.0),
        SchemeNeutral::new(seed, false, -1.0),
        SchemeRainbow::new(seed, false, 0.5),
        SchemeFruitSalad::new(seed, true, 0.0),
    ] {
        // Text keeps its contrast with what it is on.
        let (on_surface, surface) =
            (Hct::from_color(scheme.on_surface()), Hct::from_color(scheme.highest_surface_color()));
        assert!(Contrast::ratio_of_tones(on_surface.tone(), surface.tone()) >= 4.4);
    }
    // A role of one's own: a palette, a tone, kept 4.5:1 with the surface.
    let accent = DynamicColor::builder("accent")
        .palette(|s| s.tertiary_palette)
        .tone(|_| 60.0)
        .background(|s| Some(MaterialDynamicColors.highest_surface(s)))
        .contrast_curve(|_| Some(ContrastCurve::new(3.0, 4.5, 7.0, 11.0)))
        .build();
    let tone = accent.get_tone(&light);
    assert!(Contrast::ratio_of_tones(tone, MaterialDynamicColors.surface_dim().get_tone(&light)) >= 4.5 - 0.05);
    assert!(panics(|| {
        DynamicColor::builder("bad").palette(|s| s.primary_palette).background(|_| None).build();
    }));
}

trait HighestSurface {
    fn highest_surface_color(&self) -> Color;
}

impl HighestSurface for DynamicScheme {
    fn highest_surface_color(&self) -> Color {
        self.get_color(&MaterialDynamicColors.highest_surface(self))
    }
}
