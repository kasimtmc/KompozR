//! What navigation takes from the JVM as it is: a string's `hashCode` (destination ids are the hash codes of their
//! routes, typed routes' of their serial names and argument names) and the texts of `Float.toString` /
//! `Double.toString` (a typed route's float and double arguments in its path: `1.5`, `2.0`, `1.0E7`).

/// Java's `String.hashCode()`: `s[0]*31^(n-1) + … + s[n-1]` over its UTF-16 code units, wrapping.
pub(crate) fn string_hash(s: &str) -> i32 {
    s.encode_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
}

/// Java's `Double.toString(d)`: the shortest digits that read back as `d`, as `123.45` while `1e-3 <= |d| < 1e7`
/// (with a `.0` when whole), else as `1.2345E10`; `NaN`, `Infinity`, `-Infinity`, `0.0`, `-0.0`.
pub(crate) fn double_to_string(d: f64) -> String {
    if d.is_nan() {
        return "NaN".into();
    }
    if d.is_infinite() {
        return if d > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if d == 0.0 {
        return if d.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    java_decimal(&format!("{d:e}"), d.abs() >= 1e-3 && d.abs() < 1e7)
}

/// Java's `Float.toString(f)` (the same rules, the shortest digits of a float).
pub(crate) fn float_to_string(f: f32) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    java_decimal(&format!("{f:e}"), f.abs() >= 1e-3 && f.abs() < 1e7)
}

/// Rust's shortest scientific form (`-1.2345e3`) as Java writes it: plain when `plain`, else `-1.2345E3`.
fn java_decimal(sci: &str, plain: bool) -> String {
    let (mantissa, exp) = sci.split_once('e').expect("a number in scientific form");
    let exp: i32 = exp.parse().expect("an exponent");
    let (sign, mantissa) = match mantissa.strip_prefix('-') {
        Some(m) => ("-", m),
        None => ("", mantissa),
    };
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    if !plain {
        let (first, rest) = digits.split_at(1);
        let rest = if rest.is_empty() { "0" } else { rest };
        return format!("{sign}{first}.{rest}E{exp}");
    }
    // The point goes after digit `exp` (0-based): before the first one for a negative exponent.
    let point = exp + 1;
    let (int, frac) = if point <= 0 {
        ("0".to_string(), format!("{}{digits}", "0".repeat((-point) as usize)))
    } else if point as usize >= digits.len() {
        (format!("{digits}{}", "0".repeat(point as usize - digits.len())), String::new())
    } else {
        (digits[..point as usize].to_string(), digits[point as usize..].to_string())
    };
    let frac = if frac.is_empty() { "0".to_string() } else { frac };
    format!("{sign}{int}.{frac}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_codes_are_javas() {
        assert_eq!(string_hash(""), 0);
        assert_eq!(string_hash("a"), 97);
        assert_eq!(string_hash("home"), 3208415);
        // (UTF-16 code units: a character outside the BMP is two)
        assert_eq!(string_hash("ğ"), 0x11f);
        assert_eq!(string_hash("android-app://androidx.navigation/home"), 2027439596);
    }

    #[test]
    fn floats_print_as_javas() {
        assert_eq!(double_to_string(1.5), "1.5");
        assert_eq!(double_to_string(2.0), "2.0");
        assert_eq!(double_to_string(-0.25), "-0.25");
        assert_eq!(double_to_string(0.001), "0.001");
        assert_eq!(double_to_string(0.0001), "1.0E-4");
        assert_eq!(double_to_string(1e7), "1.0E7");
        assert_eq!(double_to_string(9999999.0), "9999999.0");
        assert_eq!(double_to_string(123456789.125), "1.23456789125E8");
        assert_eq!(double_to_string(0.1), "0.1");
        assert_eq!(float_to_string(0.1), "0.1");
        assert_eq!(float_to_string(2.25), "2.25");
        assert_eq!(float_to_string(1.0e10), "1.0E10");
        assert_eq!(double_to_string(f64::NAN), "NaN");
        assert_eq!(float_to_string(-0.0), "-0.0");
    }
}
