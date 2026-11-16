//! Kotlin's `String.format` (`"%.1f km".format(x)`): Java's `Formatter`, in the root locale.
//!
//! `%[index$][flags][width][.precision]conversion` with the conversions `b B s S c C d o x X e E f g G % n`, the flags
//! `- # + space 0 , (`, an argument index (`%2$s`) or the previous argument (`%<s`). Floats are rounded half up on
//! their shortest digits, as Java rounds them (`"%.1f"` of `0.15` is `0.2`); a `Float` is widened to a `Double`
//! first, as Java does. A mismatch panics with the exception Java throws.

use super::panic_kt;
use super::string::char_of;
use super::text::{FormatArg, KtToString, shortest_digits};

/// Kotlin's `String.format(format, args)` (and `format.format(args)`).
/// Kotlin: `String.format(format: String, vararg args: Any?): String`
#[track_caller]
pub fn format(format: &str, args: &[&dyn KtToString]) -> String {
    let mut out = String::new();
    let chars: Vec<char> = format.chars().collect();
    let mut i = 0;
    let mut next_arg = 0;
    let mut last_arg: Option<usize> = None;
    while i < chars.len() {
        let c = chars[i];
        if c != '%' {
            out.push(c);
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        // [index$] or <
        let mut index: Option<usize> = None;
        let mut relative = false;
        if i < chars.len() && chars[i] == '<' {
            relative = true;
            i += 1;
        } else {
            let mut j = i;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            if j > i && j < chars.len() && chars[j] == '$' {
                let n: usize = chars[i..j].iter().collect::<String>().parse().unwrap_or(0);
                index = Some(n.max(1) - 1);
                i = j + 1;
            }
        }
        let mut flags = Flags::default();
        while i < chars.len() {
            match chars[i] {
                '-' => flags.left = true,
                '#' => flags.alternate = true,
                '+' => flags.plus = true,
                ' ' => flags.space = true,
                '0' => flags.zero = true,
                ',' => flags.group = true,
                '(' => flags.paren = true,
                _ => break,
            }
            i += 1;
        }
        let mut width: Option<usize> = None;
        let w0 = i;
        while i < chars.len() && chars[i].is_ascii_digit() {
            i += 1;
        }
        if i > w0 {
            width = chars[w0..i].iter().collect::<String>().parse().ok();
        }
        let mut precision: Option<usize> = None;
        if i < chars.len() && chars[i] == '.' {
            let p0 = i + 1;
            i = p0;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            if i == p0 {
                panic_kt("IllegalFormatPrecisionException", "");
            }
            precision = chars[p0..i].iter().collect::<String>().parse().ok();
        }
        let Some(&conversion) = chars.get(i) else {
            panic_kt("UnknownFormatConversionException", "Conversion = '%'");
        };
        i += 1;
        let spec: String = chars[start..i].iter().collect();
        match conversion {
            '%' => {
                pad(&mut out, "%".to_string(), width, flags.left);
                continue;
            }
            'n' => {
                out.push('\n');
                continue;
            }
            _ => {}
        }
        let at = if relative {
            last_arg
                .unwrap_or_else(|| panic_kt("MissingFormatArgumentException", &format!("Format specifier '{spec}'")))
        } else if let Some(n) = index {
            n
        } else {
            next_arg += 1;
            next_arg - 1
        };
        let Some(arg) = args.get(at) else {
            panic_kt("MissingFormatArgumentException", &format!("Format specifier '{spec}'"));
        };
        last_arg = Some(at);
        let value = arg.kt_format_arg();
        let upper = conversion.is_ascii_uppercase();
        let text = match conversion.to_ascii_lowercase() {
            'b' => {
                let b = match value {
                    FormatArg::Null => false,
                    FormatArg::Bool(b) => b,
                    _ => true,
                };
                truncate(b.to_string(), precision)
            }
            's' => match value {
                FormatArg::Null => truncate("null".to_string(), precision),
                _ => truncate(arg.kt_to_string(), precision),
            },
            'c' => match value {
                FormatArg::Null => "null".to_string(),
                v => match char_of(v) {
                    Some(c) => c.to_string(),
                    None => conversion_error(conversion, v),
                },
            },
            'd' => match value {
                FormatArg::Int(v, _) => integer(v, &flags, width),
                FormatArg::Null => "null".to_string(),
                v => conversion_error(conversion, v),
            },
            'o' | 'x' => match value {
                FormatArg::Int(v, bits) => {
                    let unsigned = if v < 0 { (v + (1i128 << bits.min(127))) as u128 } else { v as u128 };
                    let (digits, prefix) = if conversion.eq_ignore_ascii_case(&'x') {
                        (format!("{unsigned:x}"), "0x")
                    } else {
                        (format!("{unsigned:o}"), "0")
                    };
                    let prefix = if flags.alternate { prefix } else { "" };
                    zero_pad(prefix, &digits, width, &flags)
                }
                FormatArg::Null => "null".to_string(),
                v => conversion_error(conversion, v),
            },
            'e' | 'f' | 'g' => match value {
                FormatArg::Float(v) => float(v, conversion.to_ascii_lowercase(), precision, width, &flags),
                FormatArg::Null => "null".to_string(),
                v => conversion_error(conversion, v),
            },
            _ => panic_kt("UnknownFormatConversionException", &format!("Conversion = '{conversion}'")),
        };
        let text = if upper { text.to_uppercase() } else { text };
        pad(&mut out, text, width, flags.left);
    }
    out
}

#[derive(Clone, Copy, Default)]
struct Flags {
    left: bool,
    alternate: bool,
    plus: bool,
    space: bool,
    zero: bool,
    group: bool,
    paren: bool,
}

#[track_caller]
fn conversion_error(conversion: char, value: FormatArg) -> ! {
    let ty = match value {
        FormatArg::Int(_, 8) => "java.lang.Byte",
        FormatArg::Int(_, 16) => "java.lang.Short",
        FormatArg::Int(_, 32) => "java.lang.Integer",
        FormatArg::Int(..) => "java.lang.Long",
        FormatArg::Float(_) => "java.lang.Double",
        FormatArg::Char(_) => "java.lang.Character",
        FormatArg::Bool(_) => "java.lang.Boolean",
        _ => "java.lang.String",
    };
    panic_kt("IllegalFormatConversionException", &format!("{conversion} != {ty}"))
}

fn truncate(s: String, precision: Option<usize>) -> String {
    match precision {
        Some(p) => s.chars().take(p).collect(),
        None => s,
    }
}

/// Pads to `width` with spaces, on the left (or the right, `left`-justified).
fn pad(out: &mut String, text: String, width: Option<usize>, left: bool) {
    let n = text.chars().count();
    let fill = width.map_or(0, |w| w.saturating_sub(n));
    if left {
        out.push_str(&text);
        out.extend(std::iter::repeat_n(' ', fill));
    } else {
        out.extend(std::iter::repeat_n(' ', fill));
        out.push_str(&text);
    }
}

/// Digits grouped by threes with commas (Java's `,` flag in the root locale).
fn grouped(digits: &str) -> String {
    let n = digits.len();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (n - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A number's sign and digits put together as the flags say: `+`, a space, parentheses, zeros to the width (after
/// the sign).
fn signed(negative: bool, body: &str, flags: &Flags, width: Option<usize>) -> String {
    let (lead, trail) = if negative {
        if flags.paren { ("(", ")") } else { ("-", "") }
    } else if flags.plus {
        ("+", "")
    } else if flags.space {
        (" ", "")
    } else {
        ("", "")
    };
    zero_pad_signed(lead, body, trail, width, flags)
}

fn zero_pad_signed(lead: &str, body: &str, trail: &str, width: Option<usize>, flags: &Flags) -> String {
    let mut out = lead.to_string();
    if flags.zero && !flags.left {
        let used = lead.chars().count() + body.chars().count() + trail.chars().count();
        out.extend(std::iter::repeat_n('0', width.map_or(0, |w| w.saturating_sub(used))));
    }
    out.push_str(body);
    out.push_str(trail);
    out
}

fn zero_pad(prefix: &str, digits: &str, width: Option<usize>, flags: &Flags) -> String {
    zero_pad_signed(prefix, digits, "", width, flags)
}

/// `%d`.
fn integer(v: i128, flags: &Flags, width: Option<usize>) -> String {
    let digits = v.unsigned_abs().to_string();
    let body = if flags.group { grouped(&digits) } else { digits };
    signed(v < 0, &body, flags, width)
}

/// Rounds `digits` (0.d₁d₂… × 10^(exp+1)) half up to `keep` digits: the rounded digits and exponent (which a carry
/// out of the first digit raises).
fn round_half_up(digits: &[u8], exp: i32, keep: i32) -> (Vec<u8>, i32) {
    if keep < 0 {
        return (vec![0], exp);
    }
    let keep = keep as usize;
    if digits.len() <= keep {
        let mut d = digits.to_vec();
        d.resize(keep.max(1), 0);
        return (d, exp);
    }
    let mut d: Vec<u8> = digits[..keep].to_vec();
    if digits[keep] >= 5 {
        let mut i = keep;
        loop {
            if i == 0 {
                d.insert(0, 1);
                d.truncate(keep.max(1));
                return (d, exp + 1);
            }
            i -= 1;
            if d[i] == 9 {
                d[i] = 0;
            } else {
                d[i] += 1;
                break;
            }
        }
    }
    if d.is_empty() {
        d.push(0);
    }
    (d, exp)
}

/// `value`'s digits written plain with `precision` digits after the point.
fn fixed(digits: &[u8], exp: i32, precision: usize) -> String {
    // (a keep below zero rounds to nothing: zero)
    if exp + 1 + (precision as i32) < 0 {
        return if precision == 0 { "0".to_string() } else { format!("0.{}", "0".repeat(precision)) };
    }
    let (d, e) = round_half_up(digits, exp, exp + 1 + precision as i32);
    let digit = |i: i32| -> char { if i < 0 || i as usize >= d.len() { '0' } else { (b'0' + d[i as usize]) as char } };
    let mut whole = String::new();
    if e < 0 {
        whole.push('0');
    } else {
        for i in 0..=e {
            whole.push(digit(i));
        }
    }
    let mut fraction = String::new();
    for k in 0..precision as i32 {
        // (the k-th digit after the point is at index e + 1 + k)
        fraction.push(digit(e + 1 + k));
    }
    if precision == 0 { whole } else { format!("{whole}.{fraction}") }
}

/// `value`'s digits as `d.ddd…e±xx` with `precision` digits after the point.
fn scientific(digits: &[u8], exp: i32, precision: usize, zero: bool) -> String {
    let (d, e) = if zero { (vec![0; precision + 1], 0) } else { round_half_up(digits, exp, precision as i32 + 1) };
    let mut out = String::new();
    out.push((b'0' + d[0]) as char);
    if precision > 0 {
        out.push('.');
        for k in 1..=precision {
            out.push((b'0' + d.get(k).copied().unwrap_or(0)) as char);
        }
    }
    out.push('e');
    out.push(if e < 0 { '-' } else { '+' });
    out.push_str(&format!("{:02}", e.unsigned_abs()));
    out
}

/// `%e`, `%f`, `%g`.
fn float(v: f64, conversion: char, precision: Option<usize>, width: Option<usize>, flags: &Flags) -> String {
    let negative = v.is_sign_negative() && !v.is_nan();
    if v.is_nan() || v.is_infinite() {
        let body = if v.is_nan() { "NaN" } else { "Infinity" };
        let lenient = Flags { zero: false, ..*flags };
        return signed(negative && !v.is_nan(), body, &lenient, width);
    }
    let zero = v == 0.0;
    let (digits, exp) = if zero { (vec![0u8], 0) } else { decimal_digits(v.abs()) };
    let body = match conversion {
        'f' => {
            let text = fixed(&digits, exp, precision.unwrap_or(6));
            if flags.group {
                let (w, f) = text.split_once('.').map_or((text.as_str(), None), |(w, f)| (w, Some(f)));
                match f {
                    Some(f) => format!("{}.{f}", grouped(w)),
                    None => grouped(w),
                }
            } else {
                text
            }
        }
        'e' => scientific(&digits, exp, precision.unwrap_or(6), zero),
        _ => {
            let p = match precision {
                Some(0) => 1,
                Some(p) => p,
                None => 6,
            };
            // (rounded to p significant digits first: its magnitude then picks the form)
            let (_, e) = if zero { (vec![0], 0) } else { round_half_up(&digits, exp, p as i32) };
            if zero || (-4..p as i32).contains(&e) {
                let decimals = (p as i32 - 1 - if zero { 0 } else { e }).max(0) as usize;
                fixed(&digits, exp, decimals)
            } else {
                scientific(&digits, exp, p - 1, zero)
            }
        }
    };
    signed(negative, &body, flags, width)
}

/// The shortest digits of a positive finite double as numbers 0–9, and the exponent of the first.
fn decimal_digits(v: f64) -> (Vec<u8>, i32) {
    let (digits, exp) = shortest_digits(v, false);
    (digits.bytes().map(|b| b - b'0').collect(), exp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_half_up_on_shortest_digits() {
        assert_eq!(fixed(&[1, 5], -1, 1), "0.2");
        assert_eq!(fixed(&[2, 5], -1, 1), "0.3");
        assert_eq!(fixed(&[9, 9, 9], 0, 1), "10.0");
        assert_eq!(fixed(&[4], -4, 3), "0.000");
        assert_eq!(fixed(&[5], -4, 3), "0.001");
        assert_eq!(fixed(&[5], -5, 3), "0.000");
        assert_eq!(fixed(&[1, 2, 3], 2, 0), "123");
    }
}
