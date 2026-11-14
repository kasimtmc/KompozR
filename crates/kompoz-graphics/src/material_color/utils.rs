//! Material Color Utilities' ColorUtils and MathUtils: ARGB colors (`0xAARRGGBB`, as MCU's ints) to and from
//! linear RGB, CIE XYZ (D65), L*a*b* and L*, and the arithmetic the rest shares.

/// Material Color Utilities' ColorUtils.
pub mod color_utils {
    use super::math_utils::{clamp_int, matrix_multiply};

    pub const SRGB_TO_XYZ: [[f64; 3]; 3] =
        [[0.41233895, 0.35762064, 0.18051042], [0.2126, 0.7152, 0.0722], [0.01932141, 0.11916382, 0.95034478]];

    pub const XYZ_TO_SRGB: [[f64; 3]; 3] = [
        [3.2413774792388685, -1.5376652402851851, -0.49885366846268053],
        [-0.9691452513005321, 1.8758853451067872, 0.04156585616912061],
        [0.05562093689691305, -0.20395524564742123, 1.0571799111220335],
    ];

    pub const WHITE_POINT_D65: [f64; 3] = [95.047, 100.0, 108.883];

    /// Opaque, from 0..255 channels.
    pub fn argb_from_rgb(red: i32, green: i32, blue: i32) -> u32 {
        (255 << 24 | (red & 255) << 16 | (green & 255) << 8 | blue & 255) as u32
    }

    /// From linear RGB, 0..100 each.
    pub fn argb_from_linrgb(linrgb: [f64; 3]) -> u32 {
        argb_from_rgb(delinearized(linrgb[0]), delinearized(linrgb[1]), delinearized(linrgb[2]))
    }

    pub fn alpha_from_argb(argb: u32) -> i32 {
        (argb >> 24 & 255) as i32
    }
    pub fn red_from_argb(argb: u32) -> i32 {
        (argb >> 16 & 255) as i32
    }
    pub fn green_from_argb(argb: u32) -> i32 {
        (argb >> 8 & 255) as i32
    }
    pub fn blue_from_argb(argb: u32) -> i32 {
        (argb & 255) as i32
    }

    pub fn is_opaque(argb: u32) -> bool {
        alpha_from_argb(argb) >= 255
    }

    pub fn argb_from_xyz(x: f64, y: f64, z: f64) -> u32 {
        let m = XYZ_TO_SRGB;
        let linear_r = m[0][0] * x + m[0][1] * y + m[0][2] * z;
        let linear_g = m[1][0] * x + m[1][1] * y + m[1][2] * z;
        let linear_b = m[2][0] * x + m[2][1] * y + m[2][2] * z;
        argb_from_rgb(delinearized(linear_r), delinearized(linear_g), delinearized(linear_b))
    }

    pub fn xyz_from_argb(argb: u32) -> [f64; 3] {
        let r = linearized(red_from_argb(argb));
        let g = linearized(green_from_argb(argb));
        let b = linearized(blue_from_argb(argb));
        matrix_multiply([r, g, b], SRGB_TO_XYZ)
    }

    pub fn argb_from_lab(l: f64, a: f64, b: f64) -> u32 {
        let white_point = WHITE_POINT_D65;
        let fy = (l + 16.0) / 116.0;
        let fx = a / 500.0 + fy;
        let fz = fy - b / 200.0;
        let x = lab_invf(fx) * white_point[0];
        let y = lab_invf(fy) * white_point[1];
        let z = lab_invf(fz) * white_point[2];
        argb_from_xyz(x, y, z)
    }

    pub fn lab_from_argb(argb: u32) -> [f64; 3] {
        let linear_r = linearized(red_from_argb(argb));
        let linear_g = linearized(green_from_argb(argb));
        let linear_b = linearized(blue_from_argb(argb));
        let m = SRGB_TO_XYZ;
        let x = m[0][0] * linear_r + m[0][1] * linear_g + m[0][2] * linear_b;
        let y = m[1][0] * linear_r + m[1][1] * linear_g + m[1][2] * linear_b;
        let z = m[2][0] * linear_r + m[2][1] * linear_g + m[2][2] * linear_b;
        let white_point = WHITE_POINT_D65;
        let fx = lab_f(x / white_point[0]);
        let fy = lab_f(y / white_point[1]);
        let fz = lab_f(z / white_point[2]);
        [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
    }

    /// The grey of that L*.
    pub fn argb_from_lstar(lstar: f64) -> u32 {
        let component = delinearized(y_from_lstar(lstar));
        argb_from_rgb(component, component, component)
    }

    pub fn lstar_from_argb(argb: u32) -> f64 {
        let y = xyz_from_argb(argb)[1];
        116.0 * lab_f(y / 100.0) - 16.0
    }

    /// L* to Y (0..100): the luminance of a tone.
    pub fn y_from_lstar(lstar: f64) -> f64 {
        100.0 * lab_invf((lstar + 16.0) / 116.0)
    }

    /// Y (0..100) to L*.
    pub fn lstar_from_y(y: f64) -> f64 {
        lab_f(y / 100.0) * 116.0 - 16.0
    }

    /// An 8-bit channel to linear, 0..100.
    pub fn linearized(rgb_component: i32) -> f64 {
        let normalized = rgb_component as f64 / 255.0;
        if normalized <= 0.040449936 {
            normalized / 12.92 * 100.0
        } else {
            ((normalized + 0.055) / 1.055).powf(2.4) * 100.0
        }
    }

    /// Linear, 0..100, to an 8-bit channel.
    pub fn delinearized(rgb_component: f64) -> i32 {
        let normalized = rgb_component / 100.0;
        let delinearized =
            if normalized <= 0.0031308 { normalized * 12.92 } else { 1.055 * normalized.powf(1.0 / 2.4) - 0.055 };
        clamp_int(0, 255, super::math_utils::round(delinearized * 255.0) as i32)
    }

    pub fn white_point_d65() -> [f64; 3] {
        WHITE_POINT_D65
    }

    pub fn lab_f(t: f64) -> f64 {
        let e = 216.0 / 24389.0;
        let kappa = 24389.0 / 27.0;
        if t > e { t.powf(1.0 / 3.0) } else { (kappa * t + 16.0) / 116.0 }
    }

    pub fn lab_invf(ft: f64) -> f64 {
        let e = 216.0 / 24389.0;
        let kappa = 24389.0 / 27.0;
        let ft3 = ft * ft * ft;
        if ft3 > e { ft3 } else { (116.0 * ft - 16.0) / kappa }
    }
}

/// Material Color Utilities' MathUtils, and Java's arithmetic where MCU leans on it.
pub mod math_utils {
    /// Java's Math.toDegrees and Math.toRadians: one multiplication.
    const RADIANS_TO_DEGREES: f64 = 57.29577951308232;
    const DEGREES_TO_RADIANS: f64 = 0.017453292519943295;

    pub fn to_degrees(radians: f64) -> f64 {
        radians * RADIANS_TO_DEGREES
    }
    pub fn to_radians(degrees: f64) -> f64 {
        degrees * DEGREES_TO_RADIANS
    }

    /// Java's Math.round: the nearest whole number, halves up.
    pub fn round(x: f64) -> f64 {
        let f = x.floor();
        if x - f >= 0.5 { f + 1.0 } else { f }
    }

    /// -1, 0 or 1.
    pub fn signum(num: f64) -> f64 {
        if num < 0.0 {
            -1.0
        } else if num == 0.0 {
            0.0
        } else {
            1.0
        }
    }

    /// From `start` (`amount` 0) to `stop` (1).
    pub fn lerp(start: f64, stop: f64, amount: f64) -> f64 {
        (1.0 - amount) * start + amount * stop
    }

    pub fn clamp_int(min: i32, max: i32, input: i32) -> i32 {
        if input < min {
            min
        } else if input > max {
            max
        } else {
            input
        }
    }

    pub fn clamp_double(min: f64, max: f64, input: f64) -> f64 {
        if input < min {
            min
        } else if input > max {
            max
        } else {
            input
        }
    }

    /// Within 0..360.
    pub fn sanitize_degrees_int(degrees: i32) -> i32 {
        let degrees = degrees % 360;
        if degrees < 0 { degrees + 360 } else { degrees }
    }

    /// Within 0..360 (360 out).
    pub fn sanitize_degrees_double(degrees: f64) -> f64 {
        let degrees = degrees % 360.0;
        if degrees < 0.0 { degrees + 360.0 } else { degrees }
    }

    /// 1 when the shortest way from `from` to `to` is increasing the angle, -1 otherwise.
    pub fn rotation_direction(from: f64, to: f64) -> f64 {
        let increasing_difference = sanitize_degrees_double(to - from);
        if increasing_difference <= 180.0 { 1.0 } else { -1.0 }
    }

    /// The angle between two hues, 0..180.
    pub fn difference_degrees(a: f64, b: f64) -> f64 {
        180.0 - ((a - b).abs() - 180.0).abs()
    }

    /// `row` times `matrix` (its rows).
    pub fn matrix_multiply(row: [f64; 3], matrix: [[f64; 3]; 3]) -> [f64; 3] {
        let a = row[0] * matrix[0][0] + row[1] * matrix[0][1] + row[2] * matrix[0][2];
        let b = row[0] * matrix[1][0] + row[1] * matrix[1][1] + row[2] * matrix[1][2];
        let c = row[0] * matrix[2][0] + row[1] * matrix[2][1] + row[2] * matrix[2][2];
        [a, b, c]
    }
}
