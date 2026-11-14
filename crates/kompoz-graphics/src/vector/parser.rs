//! SVG path data read into nodes, as Compose's PathParser reads it (its number reader, Compose's nextFloat, a port
//! of fast_float, included).

use super::node::PathNode;
use super::to_path::to_path_into;
use crate::path::Path;

/// Compose's PathParser: SVG path data ("M19,13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z") read into nodes it keeps. Read as
/// Compose reads it: a command letter (any letter but `e` and `E`, an exponent's) and as many whole sets of
/// numbers as follow it (a move's after the first are lines; numbers left over dropped); numbers run together where
/// they can't be one ("1.41-1.41", ".5.5"), with no `+` sign; the first arc's flags of a command one character each
/// ("a1 1 0 01.5.5"), a later set's read as numbers. Where Compose throws, at a letter it has no command for,
/// reading stops there (as the C++ Kompoz's).
#[derive(Clone, Default, Debug)]
pub struct PathParser {
    nodes: Vec<PathNode>,
}

impl PathParser {
    pub fn new() -> PathParser {
        PathParser::default()
    }

    /// Its nodes emptied (Compose's clear).
    pub fn clear(&mut self) {
        self.nodes.clear();
    }

    /// `path_data`'s nodes, in place of what it held.
    pub fn parse_path_string(&mut self, path_data: &str) -> &mut PathParser {
        self.nodes.clear();
        let mut nodes = std::mem::take(&mut self.nodes);
        self.path_string_to_nodes(path_data, &mut nodes);
        self.nodes = nodes;
        self
    }

    /// `path_data`'s nodes added to `nodes`, which it returns.
    pub fn path_string_to_nodes<'a>(&self, path_data: &str, nodes: &'a mut Vec<PathNode>) -> &'a mut Vec<PathNode> {
        let s = path_data.as_bytes();
        let (mut start, mut end) = (0, s.len());
        // Leading and trailing blanks (whatever is a space or under) trimmed.
        while start < end && s[start] <= b' ' {
            start += 1;
        }
        while end > start && s[end - 1] <= b' ' {
            end -= 1;
        }
        let mut data: Vec<f32> = Vec::with_capacity(64);
        let mut index = start;
        while index < end {
            // The next command: a letter, but not `e` or `E` (an exponent's).
            let mut command = 0u8;
            loop {
                let c = s[index];
                index += 1;
                let lower = c | 0x20;
                if lower.is_ascii_lowercase() && lower != b'e' {
                    command = c;
                    break;
                }
                if index >= end {
                    break;
                }
            }
            if command == 0 {
                continue;
            }
            if (command | 0x20) != b'z' {
                data.clear();
                // After a command letter, blanks (optional).
                while index < end && s[index] <= b' ' {
                    index += 1;
                }
                let arc = (command | 0x20) == b'a';
                loop {
                    // The arc's flags (the first set's) one character each.
                    let (next, value) = if arc && (3..=4).contains(&data.len()) {
                        next_float(s, index, (index + 1).min(end))
                    } else {
                        next_float(s, index, end)
                    };
                    index = next;
                    if !value.is_nan() {
                        data.push(value);
                    }
                    // After a number, blanks and commas, any number of each.
                    while index < end && (s[index] <= b' ' || s[index] == b',') {
                        index += 1;
                    }
                    if !(index < end && !value.is_nan()) {
                        break;
                    }
                }
            }
            if !add_nodes(command, &data, nodes) {
                break;
            }
        }
        nodes
    }

    /// `nodes` added to its nodes.
    pub fn add_path_nodes(&mut self, nodes: &[PathNode]) -> &mut PathParser {
        self.nodes.extend_from_slice(nodes);
        self
    }

    pub fn to_nodes(&self) -> &[PathNode] {
        &self.nodes
    }

    /// The path its nodes draw, a new one each time.
    pub fn to_path(&self) -> Path {
        let mut path = Path::new();
        to_path_into(&self.nodes, &mut path);
        path
    }

    /// The path its nodes draw, into `target` (emptied, its fill type kept).
    pub fn to_path_into(&self, target: &mut Path) {
        to_path_into(&self.nodes, target);
    }
}

/// SVG path data's nodes (Compose's addPathNodes).
pub fn add_path_nodes(path_str: &str) -> Vec<PathNode> {
    let mut nodes = Vec::new();
    PathParser::new().path_string_to_nodes(path_str, &mut nodes);
    nodes
}

/// The nodes of a command letter and its numbers (Compose's Char.addPathNodes): one for each whole set of them (a
/// move's after the first are lines). False for a letter of no command.
fn add_nodes(command: u8, a: &[f32], nodes: &mut Vec<PathNode>) -> bool {
    fn sets(nodes: &mut Vec<PathNode>, a: &[f32], n: usize, make: impl Fn(&[f32]) -> PathNode) {
        nodes.extend(a.chunks_exact(n).map(make));
    }
    // (Kotlin's compareTo: -0 is a set flag too.)
    let flag = |v: f32| v.to_bits() != 0;
    match command {
        b'z' | b'Z' => nodes.push(PathNode::Close),
        b'm' | b'M' => {
            if a.len() >= 2 {
                let relative = command == b'm';
                nodes.push(if relative {
                    PathNode::RelativeMoveTo { dx: a[0], dy: a[1] }
                } else {
                    PathNode::MoveTo { x: a[0], y: a[1] }
                });
                sets(nodes, &a[2..], 2, |v| {
                    if relative {
                        PathNode::RelativeLineTo { dx: v[0], dy: v[1] }
                    } else {
                        PathNode::LineTo { x: v[0], y: v[1] }
                    }
                });
            }
        }
        b'l' => sets(nodes, a, 2, |v| PathNode::RelativeLineTo { dx: v[0], dy: v[1] }),
        b'L' => sets(nodes, a, 2, |v| PathNode::LineTo { x: v[0], y: v[1] }),
        b'h' => sets(nodes, a, 1, |v| PathNode::RelativeHorizontalTo { dx: v[0] }),
        b'H' => sets(nodes, a, 1, |v| PathNode::HorizontalTo { x: v[0] }),
        b'v' => sets(nodes, a, 1, |v| PathNode::RelativeVerticalTo { dy: v[0] }),
        b'V' => sets(nodes, a, 1, |v| PathNode::VerticalTo { y: v[0] }),
        b'c' => sets(nodes, a, 6, |v| PathNode::RelativeCurveTo {
            dx1: v[0],
            dy1: v[1],
            dx2: v[2],
            dy2: v[3],
            dx3: v[4],
            dy3: v[5],
        }),
        b'C' => sets(nodes, a, 6, |v| PathNode::CurveTo { x1: v[0], y1: v[1], x2: v[2], y2: v[3], x3: v[4], y3: v[5] }),
        b's' => {
            sets(nodes, a, 4, |v| PathNode::RelativeReflectiveCurveTo { dx1: v[0], dy1: v[1], dx2: v[2], dy2: v[3] })
        }
        b'S' => sets(nodes, a, 4, |v| PathNode::ReflectiveCurveTo { x1: v[0], y1: v[1], x2: v[2], y2: v[3] }),
        b'q' => sets(nodes, a, 4, |v| PathNode::RelativeQuadTo { dx1: v[0], dy1: v[1], dx2: v[2], dy2: v[3] }),
        b'Q' => sets(nodes, a, 4, |v| PathNode::QuadTo { x1: v[0], y1: v[1], x2: v[2], y2: v[3] }),
        b't' => sets(nodes, a, 2, |v| PathNode::RelativeReflectiveQuadTo { dx: v[0], dy: v[1] }),
        b'T' => sets(nodes, a, 2, |v| PathNode::ReflectiveQuadTo { x: v[0], y: v[1] }),
        b'a' => sets(nodes, a, 7, |v| PathNode::RelativeArcTo {
            horizontal_ellipse_radius: v[0],
            vertical_ellipse_radius: v[1],
            theta: v[2],
            is_more_than_half: flag(v[3]),
            is_positive_arc: flag(v[4]),
            arc_start_dx: v[5],
            arc_start_dy: v[6],
        }),
        b'A' => sets(nodes, a, 7, |v| PathNode::ArcTo {
            horizontal_ellipse_radius: v[0],
            vertical_ellipse_radius: v[1],
            theta: v[2],
            is_more_than_half: flag(v[3]),
            is_positive_arc: flag(v[4]),
            arc_start_x: v[5],
            arc_start_y: v[6],
        }),
        _ => return false,
    }
    true
}

// ---- Compose's nextFloat --------------------------------------------------------------------------------------

/// 10^0 to 10^10, exactly (Compose's PowersOfTen).
const POWERS_OF_TEN: [f32; 11] = [1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10];

fn is_digit(c: u8) -> bool {
    c.wrapping_sub(b'0') < 10
}

/// Four digits from `offset` as a number, -1 unless all four are digits (Compose's parseFourDigits).
fn parse_four_digits(s: &[u8], offset: usize) -> i64 {
    let d = &s[offset..offset + 4];
    if d.iter().all(|&c| is_digit(c)) { d.iter().fold(0, |n, &c| n * 10 + i64::from(c - b'0')) } else { -1 }
}

/// The number at `start` (Compose's nextFloat, read no further than `end`) and where reading stopped; NaN if there
/// is none. Its grammar: an optional `-`, digits with a point among or before them (at least one digit), then an
/// optional exponent (`e` or `E`, an optional sign, digits). Its quirks kept: where it looks past `end` (the
/// character just after it; an exponent's digits), `+` not read as a sign, a lone `e` taken as an exponent of 0,
/// more than 19 digits cut to 19 (leading zeros counted unless the number ends before the data does). A number its
/// fast path can't take is the closest double to its digits, made a float (as Compose's Eisel-Lemire does), or,
/// with an exponent beyond -126..127, its text read as a float.
fn next_float(s: &[u8], start: usize, end: usize) -> (usize, f32) {
    // (Beyond the data: none, as Compose's '\u0000'.)
    let at = |i: usize| if i < s.len() { s[i] } else { 0 };
    if start >= end {
        return (start, f32::NAN);
    }
    let mut index = start;
    let mut c = at(index);
    let negative = c == b'-';
    if negative {
        index += 1;
        if index == end {
            return (index, f32::NAN);
        }
        c = at(index);
        if !is_digit(c) && c != b'.' {
            return (index, f32::NAN);
        }
    }

    // The integer part.
    let mut significand: u64 = 0;
    let significand_start = index;
    while index != end && is_digit(c) {
        significand = significand.wrapping_mul(10).wrapping_add(u64::from(c - b'0'));
        index += 1;
        c = at(index);
    }
    let significand_end = index;
    let mut digit_count = (index - significand_start) as i64;

    // The fraction.
    let mut exponent: i64 = 0;
    let mut exponent_start = index;
    let mut exponent_end = index;
    if index != end && c == b'.' {
        index += 1;
        exponent_start = index;
        while end.saturating_sub(index) >= 4 && index + 4 <= s.len() {
            let digits = parse_four_digits(s, index);
            if digits < 0 {
                break;
            }
            significand = significand.wrapping_mul(10_000).wrapping_add(digits as u64);
            index += 4;
        }
        c = at(index);
        while index != end && is_digit(c) {
            significand = significand.wrapping_mul(10).wrapping_add(u64::from(c - b'0'));
            index += 1;
            c = at(index);
        }
        exponent = exponent_start as i64 - index as i64;
        exponent_end = index;
        digit_count -= exponent;
    }
    if digit_count == 0 {
        return (index, f32::NAN);
    }

    // The exponent.
    let mut exponent_number: i64 = 0;
    if (c | 0x20) == b'e' {
        index += 1;
        c = at(index);
        let exponent_negative = c == b'-';
        if exponent_negative || c == b'+' {
            index += 1;
        }
        c = at(index);
        while index != end && is_digit(c) {
            if exponent_number < 1024 {
                exponent_number = 10 * exponent_number + i64::from(c - b'0');
            }
            index += 1;
            c = at(index);
        }
        if exponent_negative {
            exponent_number = -exponent_number;
        }
        exponent += exponent_number;
    }

    // More than 19 digits: the first 19 (after any leading zeros), the exponent moved for the rest.
    let mut too_many_digits = false;
    if digit_count > 19 {
        let mut retry = significand_start;
        c = at(retry);
        while index != end && (c == b'0' || c == b'.') {
            if c == b'0' {
                digit_count -= 1;
            }
            retry += 1;
            c = at(retry);
        }
        if digit_count > 19 {
            too_many_digits = true;
            significand = 0;
            retry = significand_start;
            c = at(retry);
            while retry != significand_end && significand < 1_000_000_000_000_000_000 {
                significand = significand.wrapping_mul(10).wrapping_add(u64::from(c.wrapping_sub(b'0')));
                retry += 1;
                c = at(retry);
            }
            if significand >= 1_000_000_000_000_000_000 {
                exponent = significand_end as i64 - retry as i64 + exponent_number;
            } else {
                retry = exponent_start;
                c = at(retry);
                while retry != exponent_end && significand < 1_000_000_000_000_000_000 {
                    significand = significand.wrapping_mul(10).wrapping_add(u64::from(c.wrapping_sub(b'0')));
                    retry += 1;
                    c = at(retry);
                }
                exponent = exponent_start as i64 - retry as i64 + exponent_number;
            }
        }
    }

    // The fast path: a significand a float holds exactly, times or over a power of ten it holds exactly.
    if (-10..=10).contains(&exponent) && !too_many_digits && significand <= 1 << 24 {
        let mut f = significand as f32;
        if exponent < 0 {
            f /= POWERS_OF_TEN[(-exponent) as usize];
        } else {
            f *= POWERS_OF_TEN[exponent as usize];
        }
        return (index, if negative { -f } else { f });
    }
    if significand == 0 {
        return (index, if negative { -0.0 } else { 0.0 });
    }
    if !(-126..=127).contains(&exponent) {
        // (Kotlin's String.toFloat of the text read; where it would throw, nothing.)
        let value = std::str::from_utf8(&s[start..index.min(s.len())])
            .ok()
            .and_then(|t| t.parse::<f32>().ok())
            .unwrap_or(f32::NAN);
        return (index, value);
    }
    let value = format!("{significand}e{exponent}").parse::<f64>().map(|d| d as f32).unwrap_or(f32::NAN);
    (index, if negative { -value } else { value })
}
